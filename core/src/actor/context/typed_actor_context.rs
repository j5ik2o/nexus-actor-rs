use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::ContextBorrow;
use crate::actor::context::{
  ActorContext, BasePart, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverPart, SenderPart,
  SpawnerPart, StopperPart,
};
use crate::actor::core::{ActorError, ActorHandle, Continuer, ExtendedPid, SpawnError, TypedExtendedPid, TypedProps};
use crate::actor::message::{
  Message, MessageHandle, ReadonlyMessageHeadersHandle, ResponseHandle, TypedMessageEnvelope,
};
use crate::actor::process::actor_future::ActorFuture;
use crate::actor::typed_context::{
  TypedContext, TypedContextSyncView, TypedInfoPart, TypedMessagePart, TypedReceiverContext, TypedReceiverPart,
  TypedSenderContext, TypedSenderPart, TypedSpawnerContext, TypedSpawnerPart, TypedStopperPart,
};
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};
use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::time::Duration;

#[derive(Debug)]
pub struct TypedActorContext<M: Message> {
  underlying: ActorContext,
  _phantom: std::marker::PhantomData<M>,
}

impl<M: Message> Clone for TypedActorContext<M> {
  fn clone(&self) -> Self {
    Self {
      underlying: self.underlying.clone(),
      _phantom: std::marker::PhantomData,
    }
  }
}

impl<M: Message> TypedActorContext<M> {
  pub fn new(underlying: ActorContext) -> Self {
    Self {
      underlying,
      _phantom: std::marker::PhantomData,
    }
  }

  pub fn get_underlying(&self) -> &ActorContext {
    &self.underlying
  }

  /// Borrows the underlying [`ActorContext`] without cloning and exposes
  /// a [`ContextBorrow`] view bound to the current lifetime.
  pub fn borrow(&self) -> ContextBorrow<'_> {
    self.underlying.borrow()
  }

  pub fn sync_view(&self) -> TypedActorContextSyncView<M> {
    TypedActorContextSyncView::new(self.clone())
  }
}

#[derive(Debug)]
pub struct TypedActorContextSyncView<M: Message> {
  context: TypedActorContext<M>,
}

impl<M: Message> TypedActorContextSyncView<M> {
  fn new(context: TypedActorContext<M>) -> Self {
    Self { context }
  }
}

impl<M: Message> TypedContextSyncView<M> for TypedActorContextSyncView<M> {
  fn actor_system_snapshot(&self) -> Option<ActorSystem> {
    Some(self.context.borrow().actor_system().clone())
  }

  fn actor_snapshot(&self) -> Option<ActorHandle> {
    self.context.borrow().actor().cloned()
  }

  fn parent_snapshot(&self) -> Option<TypedExtendedPid<M>> {
    self.context.borrow().parent().cloned().map(|pid| pid.into())
  }

  fn self_snapshot(&self) -> Option<TypedExtendedPid<M>> {
    self.context.borrow().self_pid().cloned().map(|pid| pid.into())
  }

  fn message_handle_snapshot(&self) -> Option<MessageHandle> {
    self.context.underlying.try_message_handle()
  }

  fn message_snapshot(&self) -> Option<M>
  where
    M: Clone,
  {
    self
      .context
      .underlying
      .try_message_handle()
      .and_then(|handle| handle.to_typed::<M>())
  }

  fn message_header_snapshot(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.context.underlying.try_message_header()
  }

  fn sender_snapshot(&self) -> Option<TypedExtendedPid<M>> {
    self.context.underlying.try_sender().map(|pid| pid.into())
  }
}

impl<M: Message> From<ActorContext> for TypedActorContext<M> {
  fn from(underlying: ActorContext) -> Self {
    Self::new(underlying)
  }
}

impl<M: Message> From<TypedActorContext<M>> for ActorContext {
  fn from(typed: TypedActorContext<M>) -> Self {
    typed.underlying
  }
}

impl<M: Message> ExtensionContext for TypedActorContext<M> {}

#[async_trait]
impl<M: Message> ExtensionPart for TypedActorContext<M> {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    self.underlying.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    self.underlying.set(ext).await
  }
}

impl<M: Message + Clone> TypedSenderContext<M> for TypedActorContext<M> {}

#[async_trait]
impl<M: Message> TypedInfoPart<M> for TypedActorContext<M> {
  async fn get_parent(&self) -> Option<TypedExtendedPid<M>> {
    self.underlying.borrow().parent().cloned().map(|pid| pid.into())
  }

  async fn get_self_opt(&self) -> Option<TypedExtendedPid<M>> {
    self.underlying.borrow().self_pid().cloned().map(|pid| pid.into())
  }

  async fn set_self(&mut self, pid: TypedExtendedPid<M>) {
    self.underlying.set_self(pid.into()).await;
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    self.underlying.borrow().actor().cloned()
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.underlying.borrow().actor_system().clone()
  }
}

#[async_trait]
impl<M: Message> TypedSenderPart<M> for TypedActorContext<M> {
  async fn get_sender(&self) -> Option<TypedExtendedPid<M>> {
    self.underlying.try_sender().map(|pid| pid.into())
  }

  async fn send<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A) {
    self.underlying.send(pid.into(), MessageHandle::new(message)).await;
  }

  async fn request<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A) {
    self.underlying.request(pid.into(), MessageHandle::new(message)).await;
  }

  async fn request_with_custom_sender<A: Message, B: crate::actor::message::Message>(
    &mut self,
    pid: TypedExtendedPid<A>,
    message: A,
    sender: TypedExtendedPid<B>,
  ) {
    self
      .underlying
      .request_with_custom_sender(pid.into(), MessageHandle::new(message), sender.into())
      .await;
  }

  async fn request_future<A: Message>(&self, pid: TypedExtendedPid<A>, message: A, timeout: Duration) -> ActorFuture {
    self
      .underlying
      .request_future(pid.into(), MessageHandle::new(message), timeout)
      .await
  }
}

#[async_trait]
impl<M: Message + Clone> TypedMessagePart<M> for TypedActorContext<M> {
  async fn get_message_envelope_opt(&self) -> Option<TypedMessageEnvelope<M>> {
    if let Some(envelope) = self.underlying.try_message_envelope() {
      Some(TypedMessageEnvelope::new(envelope))
    } else {
      self
        .underlying
        .get_message_envelope_opt()
        .await
        .map(|envelope| TypedMessageEnvelope::new(envelope))
    }
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    if let Some(handle) = self.underlying.try_message_handle() {
      Some(handle)
    } else {
      self.underlying.get_message_handle_opt().await
    }
  }

  async fn get_message_opt(&self) -> Option<M> {
    if let Some(handle) = self.underlying.try_message_handle() {
      handle.to_typed::<M>()
    } else {
      self
        .underlying
        .get_message_handle_opt()
        .await
        .and_then(|handle| handle.to_typed::<M>())
    }
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    if let Some(header) = self.underlying.try_message_header() {
      Some(header)
    } else {
      self.underlying.get_message_header_handle().await
    }
  }
}

impl<M: Message + Clone> TypedReceiverContext<M> for TypedActorContext<M> {}

#[async_trait]
impl<M: Message> TypedReceiverPart<M> for TypedActorContext<M> {
  async fn receive(&mut self, envelope: TypedMessageEnvelope<M>) -> Result<(), ActorError> {
    self.underlying.receive(envelope.into()).await
  }
}

impl<M: Message> TypedSpawnerContext<M> for TypedActorContext<M> {}

#[async_trait]
impl<M: Message> TypedSpawnerPart for TypedActorContext<M> {
  async fn spawn<A: Message + Clone>(&mut self, props: TypedProps<A>) -> TypedExtendedPid<A> {
    TypedExtendedPid::new(self.underlying.spawn(props.into()).await)
  }

  async fn spawn_prefix<A: Message + Clone>(&mut self, props: TypedProps<A>, prefix: &str) -> TypedExtendedPid<A> {
    TypedExtendedPid::new(self.underlying.spawn_prefix(props.into(), prefix).await)
  }

  async fn spawn_named<A: Message + Clone>(
    &mut self,
    props: TypedProps<A>,
    id: &str,
  ) -> Result<TypedExtendedPid<A>, SpawnError> {
    self
      .underlying
      .spawn_named(props.into(), id)
      .await
      .map(|pid| pid.into())
  }
}

#[async_trait]
impl<M: Message> BasePart for TypedActorContext<M> {
  fn as_any(&self) -> &dyn Any {
    self
  }

  async fn get_receive_timeout(&self) -> Duration {
    self.underlying.get_receive_timeout().await
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    BasePart::get_children(&self.underlying).await
  }

  async fn respond(&self, response: ResponseHandle) {
    self.underlying.respond(response).await
  }

  async fn stash(&mut self) {
    self.underlying.stash().await
  }

  async fn un_stash_all(&mut self) -> Result<(), ActorError> {
    self.underlying.un_stash_all().await
  }

  async fn watch(&mut self, pid: &ExtendedPid) {
    self.underlying.watch(pid).await
  }

  async fn unwatch(&mut self, pid: &ExtendedPid) {
    self.underlying.unwatch(pid).await
  }

  async fn set_receive_timeout(&mut self, d: &Duration) {
    self.underlying.set_receive_timeout(d).await
  }

  async fn cancel_receive_timeout(&mut self) {
    self.underlying.cancel_receive_timeout().await
  }

  async fn forward(&self, pid: &ExtendedPid) {
    self.underlying.forward(pid).await
  }

  async fn reenter_after(&self, f: ActorFuture, continuation: Continuer) {
    self.underlying.reenter_after(f, continuation).await
  }
}

#[async_trait]
impl<M: Message> TypedStopperPart<M> for TypedActorContext<M> {
  async fn stop(&mut self, pid: &TypedExtendedPid<M>) {
    self.underlying.stop(pid.get_underlying()).await;
  }

  async fn stop_future_with_timeout(&mut self, pid: &TypedExtendedPid<M>, timeout: Duration) -> ActorFuture {
    self
      .underlying
      .stop_future_with_timeout(pid.get_underlying(), timeout)
      .await
  }

  async fn poison(&mut self, pid: &TypedExtendedPid<M>) {
    self.underlying.poison(pid.get_underlying()).await;
  }

  async fn poison_future_with_timeout(&mut self, pid: &TypedExtendedPid<M>, timeout: Duration) -> ActorFuture {
    self
      .underlying
      .poison_future_with_timeout(pid.get_underlying(), timeout)
      .await
  }
}

impl<M: Message + Clone> TypedContext<M> for TypedActorContext<M> {}
