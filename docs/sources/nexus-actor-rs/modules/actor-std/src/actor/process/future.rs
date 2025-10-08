use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::ExtendedPid;
use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use crate::actor::metrics::metrics_impl::{MetricsRuntime, MetricsSink};
use crate::actor::process::actor_future::{ActorFuture, ActorFutureInner};
use crate::actor::process::{Process, ProcessHandle};
use crate::generated::actor::DeadLetterResponse;
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use nexus_actor_core_rs::runtime::CoreScheduledHandleRef;
use nexus_message_derive_rs::Message;
use opentelemetry::KeyValue;
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
  metrics_runtime: Arc<ArcSwapOption<MetricsRuntime>>,
  metrics_sink: Arc<ArcSwapOption<MetricsSink>>,
}

impl ActorFutureProcess {
  const METRICS_ACTOR_NAME: &'static str = "actor_future_process";

  pub async fn new(system: ActorSystem, duration: Duration) -> Arc<Self> {
    let metrics_runtime = system.metrics_runtime_slot();
    let metrics_sink = Arc::new(ArcSwapOption::from(None::<Arc<MetricsSink>>));

    let inner = Arc::new(RwLock::new(ActorFutureInner {
      actor_system: system.downgrade(),
      pid: None,
      done: false,
      result: None,
      error: None,
      pipes: Vec::new(),
      completions: Vec::new(),
      timeout_handle: None,
    }));
    let notify = Arc::new(Notify::new());

    let future = ActorFuture { inner, notify };

    let future_process = Arc::new(ActorFutureProcess {
      future: Arc::new(RwLock::new(future.clone())),
      metrics_runtime,
      metrics_sink,
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

    let pid_clone = pid.clone();
    future_process.set_pid(pid).await;

    if let Some(sink) = future_process.metrics_sink() {
      let mut labels = vec![
        KeyValue::new("future.pid", pid_clone.id().to_string()),
        KeyValue::new("future.phase", "started"),
      ];
      labels.push(KeyValue::new("future.timeout_ms", duration.as_millis() as i64));
      sink.increment_future_started_with_labels(&labels);
    }

    if duration > Duration::from_secs(0) {
      let scheduler = system.core_runtime().scheduler();
      let future_process_clone = Arc::clone(&future_process);
      let handle = scheduler.schedule_once(
        duration,
        Arc::new(move || {
          let future_process_clone = Arc::clone(&future_process_clone);
          Box::pin(async move {
            let future = future_process_clone.get_future().await;
            future.clear_timeout_handle().await;
            tracing::debug!("Future timed out");
            future_process_clone.handle_timeout().await;
          })
        }),
      );
      future_process.set_timeout_handle(handle).await;
    }

    future_process
  }

  fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    if let Some(existing) = self.metrics_sink.load_full() {
      return Some(existing);
    }
    let factory = self.metrics_runtime.load_full()?;
    let sink = Arc::new(runtime.sink_for_actor(Some(Self::METRICS_ACTOR_NAME)));
    self.metrics_sink.store(Some(sink.clone()));
    Some(sink)
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
    let future = self.get_future().await;
    future.clear_timeout_handle().await;
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
      let future_pid = future.get_pid().await;
      let (outcome_label, error_label) = {
        let actor_future_inner = future.inner.read().await;
        match &actor_future_inner.error {
          None => ("completed", None),
          Some(ActorFutureError::TimeoutError) => ("timeout", Some("timeout")),
          Some(ActorFutureError::DeadLetterError) => ("dead_letter", Some("dead_letter")),
        }
      };
      let mut labels = vec![
        KeyValue::new("future.pid", future_pid.id().to_string()),
        KeyValue::new("future.outcome", outcome_label.to_string()),
      ];
      if let Some(err) = error_label {
        labels.push(KeyValue::new("future.error", err.to_string()));
      }
      match outcome_label {
        "completed" => sink.increment_future_completed_with_labels(&labels),
        _ => sink.increment_future_timed_out_with_labels(&labels),
      }
    }
    future.instrument().await;
  }
}

impl ActorFutureProcess {
  async fn set_timeout_handle(&self, handle: CoreScheduledHandleRef) {
    let future = {
      let guard = self.future.read().await;
      guard.clone()
    };
    future.set_timeout_handle(handle).await;
  }
}

#[async_trait]
impl Process for ActorFutureProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, message_handle: MessageHandle) {
    let cloned_self = self.clone();
    let future = self.future.read().await.clone();
    let scheduler = {
      let mg = future.inner.read().await;
      mg.actor_system().core_runtime().scheduler()
    };
    let message_clone = message_handle.clone();
    scheduler.schedule_once(
      Duration::ZERO,
      Arc::new(move || {
        let future = future.clone();
        let cloned_self = cloned_self.clone();
        let message_handle = message_clone.clone();
        Box::pin(async move {
          if message_handle.to_typed::<DeadLetterResponse>().is_some() {
            future.fail(ActorFutureError::DeadLetterError).await;
          } else {
            future.complete(message_handle).await;
          }
          cloned_self.instrument(future).await;
        })
      }),
    );
  }

  async fn send_system_message(&self, _: &ExtendedPid, message_handle: MessageHandle) {
    let cloned_self = self.clone();
    let future = self.future.read().await.clone();
    let scheduler = {
      let mg = future.inner.read().await;
      mg.actor_system().core_runtime().scheduler()
    };
    let message_clone = message_handle.clone();
    scheduler.schedule_once(
      Duration::ZERO,
      Arc::new(move || {
        let future = future.clone();
        let cloned_self = cloned_self.clone();
        let message_handle = message_clone.clone();
        Box::pin(async move {
          future.complete(message_handle).await;
          cloned_self.instrument(future).await;
        })
      }),
    );
  }

  async fn stop(&self, _pid: &ExtendedPid) {}

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn Any {
    self
  }
}
