use nexus_actor_core_rs::actor::core_types::message_handle::MessageHandle;
use nexus_actor_core_rs::actor::core_types::message_headers::ReadonlyMessageHeadersHandle;
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::context::{CoreActorContext, CoreActorContextSnapshot};

use futures::FutureExt;

use crate::actor::context::{ContextHandle, InfoPart, MessagePart, SenderPart};

#[derive(Clone)]
pub struct StdActorContextSnapshot {
  core: CoreActorContextSnapshot,
}

impl StdActorContextSnapshot {
  pub fn self_pid_core(&self) -> CorePid {
    self.core.self_pid_core()
  }

  pub fn sender_pid_core(&self) -> Option<CorePid> {
    self.core.sender_pid_core()
  }

  pub fn into_core(self) -> CoreActorContextSnapshot {
    self.core
  }

  pub fn as_core(&self) -> &CoreActorContextSnapshot {
    &self.core
  }

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
      core: CoreActorContextSnapshot::new(self_pid, sender, message, headers),
    }
  }
}

impl CoreActorContext for StdActorContextSnapshot {
  fn self_pid(&self) -> CorePid {
    self.core.self_pid()
  }

  fn sender_pid(&self) -> Option<CorePid> {
    self.core.sender_pid()
  }

  fn message(&self) -> Option<MessageHandle> {
    self.core.message()
  }

  fn headers(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.core.headers()
  }
}

impl std::fmt::Debug for StdActorContextSnapshot {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("StdActorContextSnapshot")
      .field("core", &self.core)
      .finish()
  }
}
