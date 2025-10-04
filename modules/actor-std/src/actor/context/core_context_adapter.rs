use nexus_actor_core_rs::actor::core_types::message_handle::MessageHandle;
use nexus_actor_core_rs::actor::core_types::message_headers::ReadonlyMessageHeadersHandle;
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::context::{CoreActorContext, CoreActorContextSnapshot, CoreContextSnapshot};

use futures::FutureExt;

use crate::actor::context::{BasePart, ContextHandle, InfoPart, MessagePart, SenderPart};
use crate::actor::core::ActorHandle;
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct StdActorContextSnapshot {
  core: CoreContextSnapshot,
}

impl StdActorContextSnapshot {
  pub fn self_pid_core(&self) -> CorePid {
    self
      .core
      .self_pid()
      .cloned()
      .expect("StdActorContextSnapshot missing self pid")
  }

  pub fn sender_pid_core(&self) -> Option<CorePid> {
    self.core.sender_pid().cloned()
  }

  pub fn into_core(self) -> CoreActorContextSnapshot {
    self.core.build().expect("StdActorContextSnapshot missing self pid")
  }

  pub fn into_core_context(self) -> CoreContextSnapshot {
    self.core
  }

  pub fn as_core(&self) -> CoreActorContextSnapshot {
    self
      .core
      .clone()
      .build()
      .expect("StdActorContextSnapshot missing self pid")
  }

  pub async fn capture(handle: &ContextHandle) -> Self {
    let self_pid = handle.get_self().await.to_core();
    let sender = handle.get_sender().await.map(|pid| pid.to_core());
    let parent = handle.get_parent().await.map(|pid| pid.to_core());
    let message = handle
      .try_get_message_handle_opt()
      .or_else(|| handle.get_message_handle_opt().now_or_never().flatten());
    // TODO(core-context): ensure headers snapshot covers middleware mutations (may require dedicated API).
    let headers = message
      .as_ref()
      .and_then(|m| m.to_typed::<ReadonlyMessageHeadersHandle>());
    let receive_timeout: Duration = handle.get_receive_timeout().await;
    let timeout_opt = if receive_timeout.is_zero() {
      None
    } else {
      Some(receive_timeout)
    };
    let actor_arc: Option<Arc<dyn Any + Send + Sync>> = handle.get_actor().await.map(|actor| {
      let handle_arc: Arc<ActorHandle> = Arc::new(actor);
      handle_arc as Arc<dyn Any + Send + Sync>
    });
    let core = CoreContextSnapshot::new()
      .with_self_pid(Some(self_pid))
      .with_sender(sender)
      .with_parent(parent)
      .with_message_handle(message)
      .with_message_header(headers)
      .with_receive_timeout(timeout_opt)
      .with_actor(actor_arc);
    Self { core }
  }
}

impl CoreActorContext for StdActorContextSnapshot {
  fn self_pid(&self) -> CorePid {
    self.self_pid_core()
  }

  fn sender_pid(&self) -> Option<CorePid> {
    self.sender_pid_core()
  }

  fn message(&self) -> Option<MessageHandle> {
    self.core.message_handle().cloned()
  }

  fn headers(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.core.message_header().cloned()
  }

  fn receive_timeout(&self) -> Option<Duration> {
    self.core.receive_timeout()
  }
}

impl std::fmt::Debug for StdActorContextSnapshot {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("StdActorContextSnapshot")
      .field("core", &self.core)
      .finish()
  }
}
