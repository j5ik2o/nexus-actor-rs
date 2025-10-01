use nexus_actor_core_rs::actor::core_types::message_handle::MessageHandle;
use nexus_actor_core_rs::actor::core_types::message_headers::ReadonlyMessageHeadersHandle;
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::context::CoreActorContext;

use futures::FutureExt;

use crate::actor::context::{ContextHandle, InfoPart, MessagePart, SenderPart};

#[derive(Clone)]
pub struct StdActorContextSnapshot {
  self_pid: CorePid,
  sender: Option<CorePid>,
  message: Option<MessageHandle>,
  headers: Option<ReadonlyMessageHeadersHandle>,
}

impl StdActorContextSnapshot {
  pub async fn capture(handle: &ContextHandle) -> Self {
    let self_pid = handle.get_self().await.to_core();
    let sender = handle.get_sender().await.map(|pid| pid.to_core());
    let message = handle
      .try_get_message_handle_opt()
      .or_else(|| handle.get_message_handle_opt().now_or_never().flatten());
    // TODO(core-context): ensure headers snapshot covers middleware mutations (may require dedicated API).
    let headers = message
      .as_ref()
      .and_then(|m| m.to_typed::<ReadonlyMessageHeadersHandle>());
    Self {
      self_pid,
      sender,
      message,
      headers,
    }
  }
}

impl CoreActorContext for StdActorContextSnapshot {
  fn self_pid(&self) -> CorePid {
    self.self_pid.clone()
  }

  fn sender_pid(&self) -> Option<CorePid> {
    self.sender.clone()
  }

  fn message(&self) -> Option<MessageHandle> {
    self.message.clone()
  }

  fn headers(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.headers.clone()
  }
}

impl std::fmt::Debug for StdActorContextSnapshot {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("StdActorContextSnapshot")
      .field("self_pid", &self.self_pid)
      .field("sender", &self.sender)
      .finish()
  }
}
