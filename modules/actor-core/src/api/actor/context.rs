use crate::runtime::context::ActorContext;
use crate::runtime::message::DynMessage;
use crate::ActorId;
use crate::ActorPath;
use crate::MailboxFactory;
use crate::PriorityEnvelope;
use crate::Supervisor;
use crate::SystemMessage;
use alloc::{boxed::Box, sync::Arc};
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
  _marker: PhantomData<U>,
}

pub type SetupContext<'ctx, U, R> = Context<'ctx, 'ctx, U, R>;

impl<'r, 'ctx, U, R> Context<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  pub(super) fn new(inner: &'r mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>>) -> Self {
    Self {
      inner,
      _marker: PhantomData,
    }
  }

  pub(super) fn with_metadata(
    inner: &'r mut ActorContext<'ctx, DynMessage, R, dyn Supervisor<DynMessage>>,
    metadata: MessageMetadata,
  ) -> Self {
    inner.enter_metadata(metadata);
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

  pub fn message_metadata(&self) -> Option<&MessageMetadata> {
    self.inner.current_metadata()
  }

  fn self_dispatcher(&self) -> MessageDispatcher
  where
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    MessageDispatcher::from_internal_ref(self.inner.self_ref())
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
    let metadata = MessageMetadata::new(Some(self.self_dispatcher()), None);
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
    let metadata = MessageMetadata::new(Some(sender.to_dispatcher()), None);
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
      .responder_cloned()
      .or_else(|| metadata.sender_cloned())
      .ok_or(AskError::MissingResponder)?;
    let dispatch_metadata = MessageMetadata::new(Some(self.self_dispatcher()), None);
    let dyn_message = DynMessage::new(MessageEnvelope::user_with_metadata(message, dispatch_metadata));
    target.dispatch_default(dyn_message).map_err(AskError::from)?;
    Ok(())
  }

  pub fn request_future<V, Resp>(&mut self, target: &ActorRef<V, R>, message: V) -> AskResult<AskFuture<Resp>>
  where
    V: Element,
    Resp: Element,
    R::Queue<PriorityEnvelope<DynMessage>>: Clone + Send + Sync + 'static,
    R::Signal: Clone + Send + Sync + 'static, {
    let (future, responder) = create_ask_handles::<Resp>();
    let metadata = MessageMetadata::new(Some(self.self_dispatcher()), Some(responder));
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
    let metadata = MessageMetadata::new(Some(self.self_dispatcher()), Some(responder));
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
