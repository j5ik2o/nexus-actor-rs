use crate::actor::actor_system::{ActorSystem, WeakActorSystem};
use crate::actor::core::ExtendedPid;
use crate::actor::message::MessageHandle;
use crate::actor::process::future::ActorFutureError;
use nexus_actor_core_rs::runtime::CoreScheduledHandleRef;
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};

mod completion;

use completion::*;

pub(crate) struct ActorFutureInner {
  pub(crate) actor_system: WeakActorSystem,
  pub(crate) pid: Option<ExtendedPid>,
  pub(crate) done: bool,
  pub(crate) result: Option<MessageHandle>,
  pub(crate) error: Option<ActorFutureError>,
  pub(crate) pipes: Vec<ExtendedPid>,
  pub(crate) completions: Vec<Completion>,
  pub(crate) timeout_handle: Option<CoreScheduledHandleRef>,
}

static_assertions::assert_impl_all!(ActorFutureInner: Send, Sync);

impl std::fmt::Debug for ActorFutureInner {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ActorFutureInner")
      .field("pid", &self.pid)
      .field("done", &self.done)
      .field("result", &self.result)
      .field("error", &self.error)
      .field("pipes_len", &self.pipes.len())
      .field("completions_len", &self.completions.len())
      .finish()
  }
}

impl ActorFutureInner {
  pub(crate) fn actor_system(&self) -> ActorSystem {
    self
      .actor_system
      .upgrade()
      .expect("ActorSystem dropped before ActorFutureInner")
  }
}

#[derive(Debug, Clone)]
pub struct ActorFuture {
  pub(crate) inner: Arc<RwLock<ActorFutureInner>>,
  pub(crate) notify: Arc<Notify>,
}

static_assertions::assert_impl_all!(ActorFuture: Send, Sync);

impl ActorFuture {
  pub async fn result(&self) -> Result<MessageHandle, ActorFutureError> {
    loop {
      {
        let inner = self.inner.read().await;
        if inner.done {
          return if let Some(error) = &inner.error {
            Err(error.clone())
          } else {
            Ok(inner.result.as_ref().unwrap().clone())
          };
        }
      }
      self.notify.notified().await;
    }
  }

  pub async fn wait(&self) -> Option<ActorFutureError> {
    self.result().await.err()
  }

  pub async fn set_pid(&mut self, pid: ExtendedPid) {
    let mut inner = self.inner.write().await;
    inner.pid = Some(pid);
  }

  pub async fn get_pid(&self) -> ExtendedPid {
    let inner = self.inner.read().await;
    inner.pid.clone().expect("pid not set")
  }

  pub async fn pipe_to(&self, pid: ExtendedPid) {
    let mut inner = self.inner.write().await;
    inner.pipes.push(pid);
    if inner.done {
      self.send_to_pipes(&mut inner).await;
    }
  }

  pub async fn clear_timeout_handle(&self) {
    let mut inner = self.inner.write().await;
    inner.timeout_handle = None;
  }

  pub async fn set_timeout_handle(&self, handle: CoreScheduledHandleRef) {
    let mut inner = self.inner.write().await;
    inner.timeout_handle = Some(handle);
  }

  pub async fn take_timeout_handle(&self) -> Option<CoreScheduledHandleRef> {
    let mut inner = self.inner.write().await;
    inner.timeout_handle.take()
  }

  async fn send_to_pipes(&self, inner: &mut ActorFutureInner) {
    let actor_system = inner.actor_system();
    let message = if let Some(error) = &inner.error {
      MessageHandle::new(error.clone())
    } else {
      inner.result.as_ref().unwrap().clone()
    };

    for process in &inner.pipes {
      process.send_user_message(actor_system.clone(), message.clone()).await;
    }

    inner.pipes.clear();
  }

  pub async fn complete(&self, result: MessageHandle) {
    let mut inner = self.inner.write().await;
    if !inner.done {
      if let Some(handle) = inner.timeout_handle.take() {
        handle.cancel();
      }
      inner.result = Some(result);
      inner.done = true;
      self.send_to_pipes(&mut inner).await;
      self.run_completions(&mut inner).await;
      self.notify.notify_waiters();
    }
  }

  pub async fn fail(&self, error: ActorFutureError) {
    let mut inner = self.inner.write().await;
    if !inner.done {
      if let Some(handle) = inner.timeout_handle.take() {
        handle.cancel();
      }
      inner.error = Some(error);
      inner.done = true;
      self.send_to_pipes(&mut inner).await;
      self.run_completions(&mut inner).await;
      self.notify.notify_waiters();
    }
  }

  pub async fn continue_with<F, Fut>(&self, continuation: F)
  where
    F: Fn(Option<MessageHandle>, Option<ActorFutureError>) -> Fut + Send + Sync + 'static,
    Fut: core::future::Future<Output = ()> + Send + 'static, {
    let mut inner = self.inner.write().await;
    if inner.done {
      continuation(inner.result.clone(), inner.error.clone()).await;
    } else {
      inner.completions.push(completion::Completion::new(continuation));
    }
  }

  async fn run_completions(&self, inner: &mut ActorFutureInner) {
    for completion in inner.completions.drain(..) {
      completion.run(inner.result.clone(), inner.error.clone()).await;
    }
  }

  pub(crate) async fn instrument(&self) {
    // Here you would implement your metrics logging
    // This is a placeholder for the actual implementation
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let mg = self.inner.read().await;
    mg.actor_system()
  }
}
