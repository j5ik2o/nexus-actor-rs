#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::any::Any;
use core::fmt;

use crate::actor::core_types::mailbox::{CoreMailbox, CoreMailboxFuture};
use crate::actor::core_types::message_handle::MessageHandle;
use crate::actor::core_types::message_headers::ReadonlyMessageHeadersHandle;
use crate::actor::core_types::pid::CorePid;

pub trait CoreActorContext: Any + Send + Sync {
  fn self_pid(&self) -> CorePid;
  fn sender_pid(&self) -> Option<CorePid>;
  fn message(&self) -> Option<MessageHandle>;
  fn headers(&self) -> Option<ReadonlyMessageHeadersHandle>;
  // TODO(#core-context): provide accessors for receive timeout / actor state snapshots once core API stabilises.
}

#[derive(Clone)]
pub struct CoreActorContextSnapshot {
  self_pid: CorePid,
  sender: Option<CorePid>,
  message: Option<MessageHandle>,
  headers: Option<ReadonlyMessageHeadersHandle>,
}

impl CoreActorContextSnapshot {
  #[must_use]
  pub fn new(
    self_pid: CorePid,
    sender: Option<CorePid>,
    message: Option<MessageHandle>,
    headers: Option<ReadonlyMessageHeadersHandle>,
  ) -> Self {
    Self {
      self_pid,
      sender,
      message,
      headers,
    }
  }

  #[must_use]
  pub fn self_pid_core(&self) -> CorePid {
    self.self_pid.clone()
  }

  #[must_use]
  pub fn sender_pid_core(&self) -> Option<CorePid> {
    self.sender.clone()
  }

  #[must_use]
  pub fn message_handle(&self) -> Option<MessageHandle> {
    self.message.clone()
  }

  #[must_use]
  pub fn headers_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.headers.clone()
  }
}

impl CoreActorContext for CoreActorContextSnapshot {
  fn self_pid(&self) -> CorePid {
    self.self_pid_core()
  }

  fn sender_pid(&self) -> Option<CorePid> {
    self.sender_pid_core()
  }

  fn message(&self) -> Option<MessageHandle> {
    self.message_handle()
  }

  fn headers(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.headers_handle()
  }
}

impl fmt::Debug for CoreActorContextSnapshot {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CoreActorContextSnapshot")
      .field("self_pid", &self.self_pid)
      .field("sender", &self.sender)
      .finish()
  }
}

pub type CorePropsFactory = Box<dyn Fn() -> CoreProps + Send + Sync>;
pub type CoreMailboxFactory =
  Arc<dyn Fn() -> CoreMailboxFuture<'static, Arc<dyn CoreMailbox + Send + Sync>> + Send + Sync>;

#[derive(Clone, Default)]
pub struct CoreProps {
  pub actor_type: Option<alloc::sync::Arc<str>>,
  pub mailbox_factory: Option<CoreMailboxFactory>,
  // TODO(#core-context): extend with minimal actor factory / supervisor hooks required by core Actors.
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::core_types::message::Message;
  use core::any::Any;

  #[derive(Debug, Clone, PartialEq)]
  struct TestMessage(&'static str);

  impl Message for TestMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other
        .as_any()
        .downcast_ref::<TestMessage>()
        .map_or(false, |value| value == self)
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[test]
  fn snapshot_clones_core_data() {
    let self_pid = CorePid::new("node", "actor");
    let sender_pid = CorePid::new("node", "sender").with_request_id(42);
    let message = MessageHandle::new(TestMessage("hello"));

    let snapshot =
      CoreActorContextSnapshot::new(self_pid.clone(), Some(sender_pid.clone()), Some(message.clone()), None);

    assert_eq!(snapshot.self_pid_core(), self_pid);
    assert_eq!(snapshot.sender_pid_core(), Some(sender_pid));
    assert!(snapshot.message_handle().unwrap().is_typed::<TestMessage>());
    assert!(snapshot.headers_handle().is_none());

    let core_ctx: &dyn CoreActorContext = &snapshot;
    assert_eq!(core_ctx.self_pid(), self_pid);
    assert!(core_ctx.message().unwrap().is_typed::<TestMessage>());
  }
}
