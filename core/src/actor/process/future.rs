use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::ExtendedPid;
use crate::actor::dispatch::Runnable;
use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use crate::actor::metrics::metrics_impl::{MetricsSink, SyncMetricsAccess};
use crate::actor::process::actor_future::{ActorFuture, ActorFutureInner};
use crate::actor::process::{Process, ProcessHandle};
use crate::generated::actor::DeadLetterResponse;
use async_trait::async_trait;
use nexus_actor_message_derive_rs::Message;
use thiserror::Error;
use tokio::sync::{Notify, RwLock};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Message, Error)]
pub enum ActorFutureError {
  #[error("future: timeout")]
  TimeoutError,
  #[error("future: dead letter")]
  DeadLetterError,
}

#[derive(Debug, Clone)]
pub struct ActorFutureProcess {
  future: Arc<RwLock<ActorFuture>>,
  metrics_sink: Option<Arc<MetricsSink>>,
}

impl ActorFutureProcess {
  pub async fn new(system: ActorSystem, duration: Duration) -> Arc<Self> {
    let metrics_sink = system
      .metrics_runtime()
      .await
      .map(|runtime| Arc::new(runtime.sink_for_actor(Some("actor_future_process"))));

    let inner = Arc::new(RwLock::new(ActorFutureInner {
      actor_system: system.downgrade(),
      pid: None,
      done: false,
      result: None,
      error: None,
      pipes: Vec::new(),
      completions: Vec::new(),
    }));
    let notify = Arc::new(Notify::new());

    let future = ActorFuture { inner, notify };

    let future_process = Arc::new(ActorFutureProcess {
      future: Arc::new(RwLock::new(future.clone())),
      metrics_sink: metrics_sink.clone(),
    });

    let process_registry = system.get_process_registry().await;
    let id = process_registry.next_id();

    let (pid, ok) = process_registry
      .add_process(
        ProcessHandle::new_arc(future_process.clone()),
        &format!("future_{}", id),
      )
      .await;
    if !ok {
      tracing::error!("failed to register future process: pid = {}", pid);
    }

    if let Some(sink) = metrics_sink {
      sink.increment_future_started();
    }

    future_process.set_pid(pid).await;

    if duration > Duration::from_secs(0) {
      let future_process_clone = Arc::clone(&future_process);

      system
        .get_config()
        .await
        .system_dispatcher
        .schedule(Runnable::new(move || async move {
          let future = future_process_clone.get_future().await;

          tokio::select! {
              _ = future.notify.notified() => {
                tracing::debug!("Future completed");
              }
              _ = tokio::time::sleep(duration) => {
                  tracing::debug!("Future timed out");
                  future_process_clone.handle_timeout().await;
              }
          }
        }))
        .await;
    }

    future_process
  }

  fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    self.metrics_sink.clone()
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let future = self.future.read().await;
    let inner = future.inner.read().await;
    inner.actor_system()
  }

  pub async fn set_pid(&self, pid: ExtendedPid) {
    let mut future_mg = self.future.write().await;
    future_mg.set_pid(pid).await;
  }

  pub async fn get_pid(&self) -> ExtendedPid {
    let future_mg = self.future.read().await;
    future_mg.get_pid().await
  }

  pub async fn get_future(&self) -> ActorFuture {
    let future_mg = self.future.read().await;
    future_mg.clone()
  }

  pub async fn is_empty(&self) -> bool {
    let future_mg = self.future.read().await;
    let inner = future_mg.inner.read().await;
    !inner.done
  }

  pub async fn pipe_to(&self, pid: ExtendedPid) {
    let future_mg = self.future.read().await;
    future_mg.pipe_to(pid).await;
  }

  pub async fn result(&self) -> Result<MessageHandle, ActorFutureError> {
    let future_mg = self.future.read().await;
    future_mg.result().await
  }

  pub async fn complete(&self, result: MessageHandle) {
    let future_mg = self.future.read().await;
    future_mg.complete(result).await;
  }

  pub async fn fail(&self, error: ActorFutureError) {
    let future_mg = self.future.read().await;
    future_mg.fail(error).await;
  }

  async fn handle_timeout(&self) {
    let error = ActorFutureError::TimeoutError;
    self.fail(error.clone()).await;

    {
      let future = self.future.read().await;
      let mut inner = future.inner.write().await;
      for pipe in &inner.pipes {
        let actor_system = inner.actor_system();
        pipe
          .send_user_message(actor_system.clone(), MessageHandle::new(error.clone()))
          .await;
      }
      inner.pipes.clear();
    }
  }

  async fn instrument(&self, future: ActorFuture) {
    if let Some(sink) = self.metrics_sink() {
      let completed = {
        let actor_future_inner = future.inner.read().await;
        actor_future_inner.error.is_none()
      };
      if completed {
        sink.increment_future_completed();
      } else {
        sink.increment_future_timed_out();
      }
    }
    future.instrument().await;
  }
}

#[async_trait]
impl Process for ActorFutureProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, message_handle: MessageHandle) {
    let cloned_self = self.clone();
    let future = self.future.read().await.clone();
    let dispatcher = {
      let mg = future.inner.read().await;
      mg.actor_system().get_config().await.system_dispatcher.clone()
    };
    dispatcher
      .schedule(Runnable::new(move || {
        let future = future.clone();
        let cloned_self = cloned_self.clone();
        async move {
          if message_handle.to_typed::<DeadLetterResponse>().is_some() {
            future.fail(ActorFutureError::DeadLetterError).await;
          } else {
            future.complete(message_handle.clone()).await;
          }
          cloned_self.instrument(future).await;
        }
      }))
      .await;
  }

  async fn send_system_message(&self, _: &ExtendedPid, message_handle: MessageHandle) {
    let cloned_self = self.clone();
    let future = self.future.read().await.clone();
    let dispatcher = {
      let mg = future.inner.read().await;
      mg.actor_system().get_config().await.system_dispatcher.clone()
    };
    dispatcher
      .schedule(Runnable::new(move || {
        let future = future.clone();
        let cloned_self = cloned_self.clone();
        async move {
          future.complete(message_handle.clone()).await;
          cloned_self.instrument(future).await;
        }
      }))
      .await;
  }

  async fn stop(&self, _pid: &ExtendedPid) {}

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn Any {
    self
  }
}

impl SyncMetricsAccess for ActorFutureProcess {
  fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    self.metrics_sink()
  }
}
