#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use alloc::sync::Arc;
use core::any::Any;
use core::fmt;
use core::time::Duration;

use crate::actor::core_types::actor_error::CoreActorError;
use crate::actor::core_types::mailbox::{CoreMailbox, CoreMailboxFuture};
use crate::actor::core_types::message_envelope::CoreMessageEnvelope;
use crate::actor::core_types::message_handle::MessageHandle;
use crate::actor::core_types::message_headers::ReadonlyMessageHeadersHandle;
use crate::actor::core_types::pid::CorePid;
use crate::supervisor::{default_core_supervisor_strategy, CoreSupervisorStrategy};

pub trait CoreActorContext: Any + Send + Sync {
  fn self_pid(&self) -> CorePid;
  fn sender_pid(&self) -> Option<CorePid>;
  fn message(&self) -> Option<MessageHandle>;
  fn headers(&self) -> Option<ReadonlyMessageHeadersHandle>;
  fn receive_timeout(&self) -> Option<Duration> {
    None
  }
  // TODO(#core-context): provide accessors for receive timeout / actor state snapshots once core API stabilises.
}

#[derive(Clone, PartialEq, Eq)]
pub struct CoreActorContextSnapshot {
  self_pid: CorePid,
  sender: Option<CorePid>,
  message: Option<MessageHandle>,
  headers: Option<ReadonlyMessageHeadersHandle>,
  receive_timeout: Option<Duration>,
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
      receive_timeout: None,
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

  #[must_use]
  pub fn receive_timeout(&self) -> Option<Duration> {
    self.receive_timeout
  }

  #[must_use]
  pub fn with_receive_timeout(mut self, timeout: Option<Duration>) -> Self {
    self.receive_timeout = timeout;
    self
  }
}

#[derive(Clone, Debug)]
pub struct CoreActorContextBuilder {
  self_pid: CorePid,
  sender: Option<CorePid>,
  message: Option<MessageHandle>,
  headers: Option<ReadonlyMessageHeadersHandle>,
  receive_timeout: Option<Duration>,
}

impl CoreActorContextBuilder {
  #[must_use]
  pub fn new(self_pid: CorePid) -> Self {
    Self {
      self_pid,
      sender: None,
      message: None,
      headers: None,
      receive_timeout: None,
    }
  }

  #[must_use]
  pub fn with_sender(mut self, sender: Option<CorePid>) -> Self {
    self.sender = sender;
    self
  }

  #[must_use]
  pub fn with_message(mut self, message: Option<MessageHandle>) -> Self {
    self.message = message;
    self
  }

  #[must_use]
  pub fn with_headers(mut self, headers: Option<ReadonlyMessageHeadersHandle>) -> Self {
    self.headers = headers;
    self
  }

  #[must_use]
  pub fn with_receive_timeout(mut self, timeout: Option<Duration>) -> Self {
    self.receive_timeout = timeout;
    self
  }

  #[must_use]
  pub fn build(self) -> CoreActorContextSnapshot {
    CoreActorContextSnapshot {
      self_pid: self.self_pid,
      sender: self.sender,
      message: self.message,
      headers: self.headers,
      receive_timeout: self.receive_timeout,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreReceiverInvocation {
  context: CoreActorContextSnapshot,
  envelope: CoreMessageEnvelope,
}

impl CoreReceiverInvocation {
  #[must_use]
  pub fn new(context: CoreActorContextSnapshot, envelope: CoreMessageEnvelope) -> Self {
    Self { context, envelope }
  }

  #[must_use]
  pub fn context(&self) -> &CoreActorContextSnapshot {
    &self.context
  }

  #[must_use]
  pub fn envelope(&self) -> &CoreMessageEnvelope {
    &self.envelope
  }

  #[must_use]
  pub fn into_parts(self) -> (CoreActorContextSnapshot, CoreMessageEnvelope) {
    (self.context, self.envelope)
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

  fn receive_timeout(&self) -> Option<Duration> {
    self.receive_timeout()
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

pub type CoreReceiverMiddlewareChainHandle =
  crate::context::middleware::CoreReceiverMiddlewareChain<CoreReceiverInvocation, CoreActorError>;

pub type CoreActorSystemId = u64;

pub type CoreSpawnFuture<'a, T> = crate::context::middleware::CoreFuture<'a, T>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreActorSpawnError {
  AdapterUnavailable,
  AdapterFailed,
}

pub trait CoreSpawnAdapter: Any + Send + Sync {
  fn spawn<'a>(&'a self, invocation: CoreSpawnInvocation) -> CoreSpawnFuture<'a, Result<CorePid, CoreActorSpawnError>>;

  fn as_any(&self) -> &dyn Any;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreSenderSnapshot {
  context: CoreActorContextSnapshot,
  actor_system_id: CoreActorSystemId,
}

impl CoreSenderSnapshot {
  #[must_use]
  pub fn new(context: CoreActorContextSnapshot, actor_system_id: CoreActorSystemId) -> Self {
    Self {
      context,
      actor_system_id,
    }
  }

  #[must_use]
  pub fn context(&self) -> &CoreActorContextSnapshot {
    &self.context
  }

  #[must_use]
  pub fn actor_system_id(&self) -> CoreActorSystemId {
    self.actor_system_id
  }

  #[must_use]
  pub fn into_parts(self) -> (CoreActorContextSnapshot, CoreActorSystemId) {
    (self.context, self.actor_system_id)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoreSenderInvocation {
  snapshot: CoreSenderSnapshot,
  target: CorePid,
  envelope: CoreMessageEnvelope,
}

impl CoreSenderInvocation {
  #[must_use]
  pub fn new(snapshot: CoreSenderSnapshot, target: CorePid, envelope: CoreMessageEnvelope) -> Self {
    Self {
      snapshot,
      target,
      envelope,
    }
  }

  #[must_use]
  pub fn snapshot(&self) -> &CoreSenderSnapshot {
    &self.snapshot
  }

  #[must_use]
  pub fn target(&self) -> &CorePid {
    &self.target
  }

  #[must_use]
  pub fn envelope(&self) -> &CoreMessageEnvelope {
    &self.envelope
  }

  #[must_use]
  pub fn into_parts(self) -> (CoreSenderSnapshot, CorePid, CoreMessageEnvelope) {
    (self.snapshot, self.target, self.envelope)
  }
}

pub type CoreSenderMiddlewareChainHandle = crate::context::middleware::CoreSenderMiddlewareChain<CoreSenderInvocation>;

#[derive(Clone)]
pub struct CoreSpawnInvocation {
  parent: CoreSenderSnapshot,
  child_props: CoreProps,
  child_pid: CorePid,
  metadata: Option<alloc::sync::Arc<str>>,
  adapter: Arc<dyn CoreSpawnAdapter>,
}

impl CoreSpawnInvocation {
  #[must_use]
  pub fn new(
    parent: CoreSenderSnapshot,
    child_props: CoreProps,
    child_pid: CorePid,
    metadata: Option<alloc::sync::Arc<str>>,
    adapter: Arc<dyn CoreSpawnAdapter>,
  ) -> Self {
    Self {
      parent,
      child_props,
      child_pid,
      metadata,
      adapter,
    }
  }

  #[must_use]
  pub fn parent(&self) -> &CoreSenderSnapshot {
    &self.parent
  }

  #[must_use]
  pub fn child_props(&self) -> &CoreProps {
    &self.child_props
  }

  #[must_use]
  pub fn child_pid(&self) -> &CorePid {
    &self.child_pid
  }

  #[must_use]
  pub fn metadata(&self) -> Option<&alloc::sync::Arc<str>> {
    self.metadata.as_ref()
  }

  #[must_use]
  pub fn adapter(&self) -> Arc<dyn CoreSpawnAdapter> {
    Arc::clone(&self.adapter)
  }

  #[must_use]
  pub fn into_parts(
    self,
  ) -> (
    CoreSenderSnapshot,
    CoreProps,
    CorePid,
    Option<alloc::sync::Arc<str>>,
    Arc<dyn CoreSpawnAdapter>,
  ) {
    (
      self.parent,
      self.child_props,
      self.child_pid,
      self.metadata,
      self.adapter,
    )
  }
}

impl fmt::Debug for CoreSpawnInvocation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CoreSpawnInvocation")
      .field("parent", &self.parent)
      .field("child_props", &self.child_props)
      .field("child_pid", &self.child_pid)
      .field("metadata", &self.metadata)
      .field("has_adapter", &true)
      .finish()
  }
}

pub type CoreSpawnMiddlewareChainHandle = crate::context::middleware::CoreSpawnMiddlewareChain;

#[derive(Clone)]
pub struct CoreProps {
  pub actor_type: Option<alloc::sync::Arc<str>>,
  pub mailbox_factory: Option<CoreMailboxFactory>,
  pub supervisor_strategy: Option<CoreSupervisorStrategyHandle>,
  pub receiver_middleware_chain: Option<CoreReceiverMiddlewareChainHandle>,
  pub sender_middleware_chain: Option<CoreSenderMiddlewareChainHandle>,
  pub spawn_middleware_chain: Option<CoreSpawnMiddlewareChainHandle>,
  pub spawn_adapter: Option<Arc<dyn CoreSpawnAdapter>>,
  // TODO(#core-context): extend with minimal actor factory / supervisor hooks required by core Actors.
}

impl fmt::Debug for CoreProps {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CoreProps")
      .field("actor_type", &self.actor_type)
      .field("has_mailbox_factory", &self.mailbox_factory.is_some())
      .field("has_supervisor_strategy", &self.supervisor_strategy.is_some())
      .field("has_receiver_middleware", &self.receiver_middleware_chain.is_some())
      .field("has_sender_middleware", &self.sender_middleware_chain.is_some())
      .field("has_spawn_middleware", &self.spawn_middleware_chain.is_some())
      .field("has_spawn_adapter", &self.spawn_adapter.is_some())
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
  pub fn with_receiver_middleware_chain(mut self, chain: CoreReceiverMiddlewareChainHandle) -> Self {
    self.receiver_middleware_chain = Some(chain);
    self
  }

  #[must_use]
  pub fn with_sender_middleware_chain(mut self, chain: CoreSenderMiddlewareChainHandle) -> Self {
    self.sender_middleware_chain = Some(chain);
    self
  }

  #[must_use]
  pub fn with_spawn_middleware_chain(mut self, chain: CoreSpawnMiddlewareChainHandle) -> Self {
    self.spawn_middleware_chain = Some(chain);
    self
  }

  #[must_use]
  pub fn with_spawn_adapter(mut self, adapter: Arc<dyn CoreSpawnAdapter>) -> Self {
    self.spawn_adapter = Some(adapter);
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

  #[must_use]
  pub fn receiver_middleware_chain(&self) -> Option<&CoreReceiverMiddlewareChainHandle> {
    self.receiver_middleware_chain.as_ref()
  }

  #[must_use]
  pub fn sender_middleware_chain(&self) -> Option<&CoreSenderMiddlewareChainHandle> {
    self.sender_middleware_chain.as_ref()
  }

  #[must_use]
  pub fn spawn_middleware_chain(&self) -> Option<&CoreSpawnMiddlewareChainHandle> {
    self.spawn_middleware_chain.as_ref()
  }

  #[must_use]
  pub fn spawn_adapter(&self) -> Option<Arc<dyn CoreSpawnAdapter>> {
    self.spawn_adapter.as_ref().map(Arc::clone)
  }
}

impl Default for CoreProps {
  fn default() -> Self {
    Self {
      actor_type: None,
      mailbox_factory: None,
      supervisor_strategy: None,
      receiver_middleware_chain: None,
      sender_middleware_chain: None,
      spawn_middleware_chain: None,
      spawn_adapter: None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::core_types::message::Message;
  use alloc::sync::Arc;
  use core::any::Any;
  use core::time::Duration;

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
    assert!(snapshot.receive_timeout().is_none());

    let core_ctx: &dyn CoreActorContext = &snapshot;
    assert_eq!(core_ctx.self_pid(), self_pid);
    assert!(core_ctx.message().unwrap().is_typed::<TestMessage>());
  }

  #[test]
  fn builder_supports_receive_timeout() {
    let snapshot = CoreActorContextBuilder::new(CorePid::new("node", "actor"))
      .with_receive_timeout(Some(Duration::from_secs(5)))
      .build();

    assert_eq!(snapshot.receive_timeout(), Some(Duration::from_secs(5)));
  }

  #[test]
  fn supervisor_strategy_handle_prefers_custom_strategy() {
    use crate::supervisor::{
      CoreSupervisor, CoreSupervisorContext, CoreSupervisorDirective, CoreSupervisorFuture, CoreSupervisorStrategy,
      CoreSupervisorStrategyFuture,
    };
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

      fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
        self
      }
    }

    #[derive(Default)]
    struct NullContext;

    impl CoreSupervisorContext for NullContext {
      fn now(&self) -> u64 {
        0
      }

      fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
        self
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

  #[test]
  fn receiver_invocation_roundtrip() {
    let context = CoreActorContextSnapshot::new(CorePid::new("node", "receiver"), None, None, None);
    let envelope = CoreMessageEnvelope::new(MessageHandle::new(TestMessage("payload")));
    let invocation = CoreReceiverInvocation::new(context.clone(), envelope.clone());
    let (ctx, env) = invocation.clone().into_parts();
    assert_eq!(ctx.self_pid_core(), context.self_pid_core());
    assert_eq!(env.message_handle().type_id(), envelope.message_handle().type_id());
    assert_eq!(invocation.context().self_pid_core(), context.self_pid_core());
  }

  #[test]
  fn receiver_middleware_chain_defaults_and_setters() {
    let props = CoreProps::default();
    assert!(props.receiver_middleware_chain().is_none());
    assert!(props.sender_middleware_chain().is_none());
    assert!(props.spawn_middleware_chain().is_none());
    assert!(props.spawn_adapter().is_none());

    let chain: CoreReceiverMiddlewareChainHandle =
      crate::context::middleware::CoreReceiverMiddlewareChain::new(|_| async { Ok::<_, CoreActorError>(()) });
    let configured = CoreProps::new().with_receiver_middleware_chain(chain.clone());
    assert!(configured.receiver_middleware_chain().is_some());
    assert_eq!(configured.receiver_middleware_chain().unwrap(), &chain);

    let sender_chain: CoreSenderMiddlewareChainHandle =
      crate::context::middleware::CoreSenderMiddlewareChain::new(|_| async {});
    let configured = configured.with_sender_middleware_chain(sender_chain.clone());
    assert!(configured.sender_middleware_chain().is_some());
    assert_eq!(configured.sender_middleware_chain().unwrap(), &sender_chain);

    struct DummyAdapter;

    impl CoreSpawnAdapter for DummyAdapter {
      fn as_any(&self) -> &dyn Any {
        self
      }

      fn spawn<'a>(
        &'a self,
        _invocation: CoreSpawnInvocation,
      ) -> CoreSpawnFuture<'a, Result<CorePid, CoreActorSpawnError>> {
        Box::pin(async { Err(CoreActorSpawnError::AdapterUnavailable) })
      }
    }

    let spawn_chain = crate::context::middleware::CoreSpawnMiddlewareChain::new(|_| {
      Box::pin(async { Err(CoreActorSpawnError::AdapterUnavailable) })
    });

    let configured = configured
      .with_spawn_middleware_chain(spawn_chain)
      .with_spawn_adapter(Arc::new(DummyAdapter));

    assert!(configured.spawn_middleware_chain().is_some());
    assert!(configured.spawn_adapter().is_some());
  }
}
