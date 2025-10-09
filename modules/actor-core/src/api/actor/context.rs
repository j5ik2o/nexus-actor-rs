use crate::runtime::context::ActorContext;
use crate::runtime::message::DynMessage;
use crate::ActorId;
use crate::ActorPath;
use crate::MailboxFactory;
use crate::PriorityEnvelope;
use crate::Supervisor;
use crate::SystemMessage;
use alloc::{boxed::Box, string::String, sync::Arc};
use core::future::Future;
use core::marker::PhantomData;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

use super::{
  ask::create_ask_handles, ask_with_timeout, ActorRef, AskError, AskFuture, AskResult, AskTimeoutFuture, Props,
};
use crate::api::{MessageDispatcher, MessageEnvelope, MessageMetadata};

/// Typed actor execution context wrapper.
/// 'r: lifetime of the mutable reference to ActorContext
/// 'ctx: lifetime parameter of ActorContext itself
pub struct Context<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  inner: &'r mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>>,
  metadata: Option<MessageMetadata>,
  _marker: PhantomData<U>,
}

pub type SetupContext<'ctx, U, R> = Context<'ctx, 'ctx, U, R>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextLogLevel {
  Trace,
  Debug,
  Info,
  Warn,
  Error,
}

#[derive(Clone)]
pub struct ContextLogger {
  actor_id: ActorId,
  actor_path: ActorPath,
}

impl ContextLogger {
  pub(crate) fn new(actor_id: ActorId, actor_path: &ActorPath) -> Self {
    Self {
      actor_id,
      actor_path: actor_path.clone(),
    }
  }

  pub fn actor_id(&self) -> ActorId {
    self.actor_id
  }

  pub fn actor_path(&self) -> &ActorPath {
    &self.actor_path
  }

  pub fn trace<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Trace, message);
  }

  pub fn debug<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Debug, message);
  }

  pub fn info<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Info, message);
  }

  pub fn warn<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Warn, message);
  }

  pub fn error<F>(&self, message: F)
  where
    F: FnOnce() -> String, {
    self.emit(ContextLogLevel::Error, message);
  }

  fn emit<F>(&self, level: ContextLogLevel, message: F)
  where
    F: FnOnce() -> String, {
    #[cfg(feature = "tracing")]
    {
      let text = message();
      match level {
        ContextLogLevel::Trace => tracing::event!(
          target: "nexus::actor",
          tracing::Level::TRACE,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Debug => tracing::event!(
          target: "nexus::actor",
          tracing::Level::DEBUG,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Info => tracing::event!(
          target: "nexus::actor",
          tracing::Level::INFO,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Warn => tracing::event!(
          target: "nexus::actor",
          tracing::Level::WARN,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
        ContextLogLevel::Error => tracing::event!(
          target: "nexus::actor",
          tracing::Level::ERROR,
          actor_id = %self.actor_id,
          actor_path = %self.actor_path,
          message = %text
        ),
      }
    }

    #[cfg(not(feature = "tracing"))]
    {
      let _ = level;
      let _ = message;
    }
  }
}

impl<'r, 'ctx, U, R> Context<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  pub(super) fn new(inner: &'r mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>>) -> Self {
    let metadata = inner.take_typed_metadata();
    Self {
      inner,
      metadata,
      _marker: PhantomData,
    }
  }

  pub fn message_metadata(&self) -> Option<&MessageMetadata> {
    self.metadata.as_ref()
  }

  pub(super) fn with_metadata(
    inner: &'r mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>>,
    metadata: MessageMetadata,
  ) -> Self {
    inner.enter_typed_metadata(metadata);
    Self::new(inner)
  }

  pub fn actor_id(&self) -> ActorId {
    self.inner.actor_id()
  }

  pub fn actor_path(&self) -> &ActorPath {
    self.inner.actor_path()
  }

  pub fn watchers(&self) -> &[ActorId] {
    self.inner.watchers()
  }

  pub fn log(&self) -> ContextLogger {
    ContextLogger::new(self.actor_id(), self.actor_path())
  }

  pub fn send_to_self(&self, message: U) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let dyn_message = DynMessage::new(MessageEnvelope::user(message));
    self.inner.send_to_self_with_priority(dyn_message, DEFAULT_PRIORITY)
  }

  pub fn send_system_to_self(&self, message: SystemMessage) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let envelope =
      PriorityEnvelope::from_system(message.clone()).map(|sys| DynMessage::new(MessageEnvelope::<U>::System(sys)));
    self.inner.send_envelope_to_self(envelope)
  }

  pub fn self_ref(&self) -> ActorRef<U, R> {
    ActorRef::new(self.inner.self_ref())
  }

  pub fn message_adapter<Ext, F>(&self, f: F) -> MessageAdapterRef<Ext, U, R>
  where
    Ext: Element,
    F: Fn(Ext) -> U + Send + Sync + 'static, {
    MessageAdapterRef::new(self.self_ref(), Arc::new(f))
  }

  pub fn register_watcher(&mut self, watcher: ActorId) {
    self.inner.register_watcher(watcher);
  }

  pub fn unregister_watcher(&mut self, watcher: ActorId) {
    self.inner.unregister_watcher(watcher);
  }

  pub fn inner(&mut self) -> &mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>> {
    self.inner
  }

  fn self_dispatcher(&self) -> MessageDispatcher<U>
  where
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    self.self_ref().to_dispatcher()
  }

  pub fn request<V>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    V: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let metadata = MessageMetadata::new().with_sender(self.self_dispatcher());
    target.tell_with_metadata(message, metadata)
  }

  pub fn request_with_sender<V, S>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
    sender: &ActorRef<S, R>,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    V: Element,
    S: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let metadata = MessageMetadata::new().with_sender(sender.to_dispatcher());
    target.tell_with_metadata(message, metadata)
  }

  pub fn forward<V>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
  ) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>>
  where
    V: Element, {
    let metadata = self.message_metadata().cloned().unwrap_or_default();
    target.tell_with_metadata(message, metadata)
  }

  pub fn respond<Resp>(&mut self, message: Resp) -> AskResult<()>
  where
    Resp: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let metadata = self.message_metadata().ok_or(AskError::MissingResponder)?;
    let target = metadata
      .responder_as::<Resp>()
      .or_else(|| metadata.sender_as::<Resp>())
      .ok_or(AskError::MissingResponder)?;
    let dispatch_metadata = MessageMetadata::new().with_sender(self.self_dispatcher());
    let envelope = MessageEnvelope::user_with_metadata(message, dispatch_metadata);
    target.dispatch_envelope(envelope).map_err(AskError::from)?;
    Ok(())
  }

  pub fn request_future<V, Resp>(&mut self, target: &ActorRef<V, R>, message: V) -> AskResult<AskFuture<Resp>>
  where
    V: Element,
    Resp: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new()
      .with_sender(self.self_dispatcher())
      .with_responder(responder);
    target.tell_with_metadata(message, metadata)?;
    Ok(future)
  }

  pub fn request_future_with_timeout<V, Resp, TFut>(
    &mut self,
    target: &ActorRef<V, R>,
    message: V,
    timeout: TFut,
  ) -> AskResult<AskTimeoutFuture<Resp, TFut>>
  where
    V: Element,
    Resp: Element,
    TFut: Future<Output = ()> + Unpin,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let mut timeout = Some(timeout);
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new()
      .with_sender(self.self_dispatcher())
      .with_responder(responder);
    match target.tell_with_metadata(message, metadata) {
      Ok(()) => Ok(ask_with_timeout(future, timeout.take().unwrap())),
      Err(err) => Err(AskError::from(err)),
    }
  }

  /// 子アクターを生成し、`ActorRef` を返す。
  pub fn spawn_child<V>(&mut self, props: Props<V, R>) -> ActorRef<V, R>
  where
    V: Element, {
    let (internal_props, supervisor_cfg) = props.into_parts();
    let actor_ref = self
      .inner
      .spawn_child_from_props(Box::new(supervisor_cfg.into_supervisor()), internal_props);
    ActorRef::new(actor_ref)
  }
}

impl<'r, 'ctx, U, R> Drop for Context<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  fn drop(&mut self) {
    self.inner.clear_metadata();
  }
}

#[derive(Clone)]
pub struct MessageAdapterRef<Ext, U, R>
where
  Ext: Element,
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  target: ActorRef<U, R>,
  adapter: Arc<dyn Fn(Ext) -> U + Send + Sync>,
}

impl<Ext, U, R> MessageAdapterRef<Ext, U, R>
where
  Ext: Element,
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  pub(crate) fn new(target: ActorRef<U, R>, adapter: Arc<dyn Fn(Ext) -> U + Send + Sync>) -> Self {
    Self { target, adapter }
  }

  pub fn tell(&self, message: Ext) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let mapped = (self.adapter)(message);
    self.target.tell(mapped)
  }

  pub fn tell_with_priority(&self, message: Ext, priority: i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let mapped = (self.adapter)(message);
    self.target.tell_with_priority(mapped, priority)
  }

  pub fn target(&self) -> &ActorRef<U, R> {
    &self.target
  }
}
