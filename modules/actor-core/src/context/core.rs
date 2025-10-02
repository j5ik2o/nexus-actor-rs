#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::any::Any;
use core::fmt;

use crate::actor::core_types::mailbox::{CoreMailbox, CoreMailboxFuture};
use crate::actor::core_types::message_handle::MessageHandle;
use crate::actor::core_types::message_headers::ReadonlyMessageHeadersHandle;
use crate::actor::core_types::pid::CorePid;
use crate::supervisor::{default_core_supervisor_strategy, CoreSupervisorStrategy};

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
pub type CoreSupervisorStrategyHandle = Arc<dyn CoreSupervisorStrategy + Send + Sync>;

#[derive(Clone, Default)]
pub struct CoreProps {
  pub actor_type: Option<alloc::sync::Arc<str>>,
  pub mailbox_factory: Option<CoreMailboxFactory>,
  pub supervisor_strategy: Option<CoreSupervisorStrategyHandle>,
  // TODO(#core-context): extend with minimal actor factory / supervisor hooks required by core Actors.
}

impl fmt::Debug for CoreProps {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CoreProps")
      .field("actor_type", &self.actor_type)
      .field("has_mailbox_factory", &self.mailbox_factory.is_some())
      .field("has_supervisor_strategy", &self.supervisor_strategy.is_some())
      .finish()
  }
}

impl CoreProps {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use]
  pub fn with_actor_type(mut self, actor_type: Option<alloc::sync::Arc<str>>) -> Self {
    self.actor_type = actor_type;
    self
  }

  #[must_use]
  pub fn with_mailbox_factory(mut self, factory: CoreMailboxFactory) -> Self {
    self.mailbox_factory = Some(factory);
    self
  }

  #[must_use]
  pub fn with_supervisor_strategy(mut self, strategy: CoreSupervisorStrategyHandle) -> Self {
    self.supervisor_strategy = Some(strategy);
    self
  }

  #[must_use]
  pub fn mailbox_factory(&self) -> Option<&CoreMailboxFactory> {
    self.mailbox_factory.as_ref()
  }

  #[must_use]
  pub fn supervisor_strategy_handle(&self) -> CoreSupervisorStrategyHandle {
    self
      .supervisor_strategy
      .clone()
      .unwrap_or_else(default_core_supervisor_strategy)
  }
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

  #[test]
  fn supervisor_strategy_handle_prefers_custom_strategy() {
    use crate::supervisor::{
      CoreSupervisor, CoreSupervisorContext, CoreSupervisorDirective, CoreSupervisorFuture, CoreSupervisorStrategy,
      CoreSupervisorStrategyFuture,
    };
    use alloc::sync::Arc;
    use alloc::vec::Vec;

    #[derive(Clone)]
    struct DummyStrategy;

    impl CoreSupervisorStrategy for DummyStrategy {
      fn handle_child_failure<'a>(
        &'a self,
        _ctx: &'a dyn CoreSupervisorContext,
        _supervisor: &'a dyn CoreSupervisor,
        _child: CorePid,
        _tracker: &'a mut crate::actor::core_types::restart::CoreRestartTracker,
        _reason: crate::error::ErrorReasonCore,
        _message: MessageHandle,
      ) -> CoreSupervisorStrategyFuture<'a> {
        Box::pin(async move {})
      }
    }

    #[derive(Default)]
    struct NullSupervisor;

    impl CoreSupervisor for NullSupervisor {
      fn children<'a>(&'a self) -> CoreSupervisorFuture<'a, Vec<CorePid>> {
        Box::pin(async { Vec::new() })
      }

      fn apply_directive<'a>(
        &'a self,
        _: CoreSupervisorDirective,
        _: &'a [CorePid],
        _: crate::error::ErrorReasonCore,
        _: MessageHandle,
      ) -> CoreSupervisorFuture<'a, ()> {
        Box::pin(async {})
      }

      fn escalate<'a>(&'a self, _: crate::error::ErrorReasonCore, _: MessageHandle) -> CoreSupervisorFuture<'a, ()> {
        Box::pin(async {})
      }
    }

    #[derive(Default)]
    struct NullContext;

    impl CoreSupervisorContext for NullContext {
      fn now(&self) -> u64 {
        0
      }
    }

    let custom: CoreSupervisorStrategyHandle = Arc::new(DummyStrategy);
    let props = CoreProps::new().with_supervisor_strategy(custom.clone());

    let handle = props.supervisor_strategy_handle();
    assert!(Arc::ptr_eq(&handle, &custom));

    // Ensure obtained strategy future is executable without panic.
    let mut tracker = crate::actor::core_types::restart::CoreRestartTracker::new();
    let context = NullContext::default();
    let supervisor = NullSupervisor::default();
    let mut future = handle.handle_child_failure(
      &context,
      &supervisor,
      CorePid::new("node", "child"),
      &mut tracker,
      crate::error::ErrorReasonCore::new("test", 0),
      MessageHandle::new(TestMessage("msg")),
    );

    fn poll_ready(future: &mut CoreSupervisorStrategyFuture<'_>) {
      use core::task::{Context, RawWaker, RawWakerVTable, Waker};

      fn noop_clone(_: *const ()) -> RawWaker {
        RawWaker::new(core::ptr::null(), &VTABLE)
      }
      fn noop(_: *const ()) {}
      static VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);

      let raw_waker = RawWaker::new(core::ptr::null(), &VTABLE);
      let waker = unsafe { Waker::from_raw(raw_waker) };
      let mut cx = Context::from_waker(&waker);
      assert!(future.as_mut().poll(&mut cx).is_ready());
    }

    poll_ready(&mut future);
  }
}
