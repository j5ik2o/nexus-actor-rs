use nexus_actor_core_rs::actor::core_types::message_handle::MessageHandle;
use nexus_actor_core_rs::actor::core_types::message_headers::ReadonlyMessageHeadersHandle;
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::context::{CoreActorContext, CoreActorContextBuilder, CoreActorContextSnapshot};

use futures::FutureExt;

use crate::actor::context::{BasePart, ContextHandle, InfoPart, MessagePart, SenderPart};
use std::time::Duration;

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
    let receive_timeout: Duration = handle.get_receive_timeout().await;
    let timeout_opt = if receive_timeout.is_zero() {
      None
    } else {
      Some(receive_timeout)
    };
    let core = CoreActorContextBuilder::new(self_pid)
      .with_sender(sender)
      .with_message(message)
      .with_headers(headers)
      .with_receive_timeout(timeout_opt)
      .build();
    Self { core }
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
