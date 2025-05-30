use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{
  BasePart, ContextHandle, ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverPart, SenderPart,
  SpawnerPart, StopperPart,
};
use crate::actor::core::{ActorError, ActorHandle, Continuer, ExtendedPid, SpawnError, TypedExtendedPid, TypedProps};
use crate::actor::message::{
  Message, MessageHandle, ReadonlyMessageHeadersHandle, ResponseHandle, TypedMessageEnvelope,
};
use crate::actor::process::actor_future::ActorFuture;
use crate::actor::typed_context::{
  TypedContext, TypedInfoPart, TypedMessagePart, TypedReceiverContext, TypedReceiverPart, TypedSenderContext,
  TypedSenderPart, TypedSpawnerContext, TypedSpawnerPart, TypedStopperPart,
};
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};
use async_trait::async_trait;
use std::any::Any;
use std::marker::PhantomData;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct TypedContextHandle<M: Message> {
  underlying: ContextHandle,
  phantom_data: PhantomData<M>,
}

impl<M: Message> TypedContextHandle<M> {
  pub fn new(underlying: ContextHandle) -> Self {
    Self {
      underlying,
      phantom_data: PhantomData,
    }
  }

  pub fn get_underlying(&self) -> &ContextHandle {
    &self.underlying
  }
}

impl<M: Message> ExtensionContext for TypedContextHandle<M> {}

#[async_trait]
impl<M: Message> ExtensionPart for TypedContextHandle<M> {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    self.underlying.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    self.underlying.set(ext).await
  }
}

impl<M: Message + Clone> TypedSenderContext<M> for TypedContextHandle<M> {}

#[async_trait]
impl<M: Message> TypedInfoPart<M> for TypedContextHandle<M> {
  async fn get_parent(&self) -> Option<TypedExtendedPid<M>> {
    self.underlying.get_parent().await.map(|pid| pid.into())
  }

  async fn get_self_opt(&self) -> Option<TypedExtendedPid<M>> {
    self.underlying.get_self_opt().await.map(|pid| pid.into())
  }

  async fn set_self(&mut self, pid: TypedExtendedPid<M>) {
    self.underlying.set_self(pid.into()).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    self.underlying.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.underlying.get_actor_system().await
  }
}

#[async_trait]
impl<M: Message> TypedSenderPart<M> for TypedContextHandle<M> {
  async fn get_sender(&self) -> Option<TypedExtendedPid<M>> {
    self.underlying.get_sender().await.map(|pid| pid.into())
  }

  async fn send<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A) {
    self.underlying.send(pid.into(), MessageHandle::new(message)).await
  }

  async fn request<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A) {
    self.underlying.request(pid.into(), MessageHandle::new(message)).await
  }

  async fn request_with_custom_sender<A: Message, B: Message>(
    &mut self,
    pid: TypedExtendedPid<A>,
    message: A,
    sender: TypedExtendedPid<B>,
  ) {
    self
      .underlying
      .request_with_custom_sender(pid.into(), MessageHandle::new(message), sender.into())
      .await
  }

  async fn request_future<A: Message>(&self, pid: TypedExtendedPid<A>, message: A, timeout: Duration) -> ActorFuture {
    self
      .underlying
      .request_future(pid.into(), MessageHandle::new(message), timeout)
      .await
  }
}

#[async_trait]
impl<M: Message + Clone> TypedMessagePart<M> for TypedContextHandle<M> {
  async fn get_message_envelope_opt(&self) -> Option<TypedMessageEnvelope<M>> {
    self
      .underlying
      .get_message_envelope_opt()
      .await
      .map(|envelope| TypedMessageEnvelope::new(envelope))
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    self.underlying.get_message_handle_opt().await
  }

  async fn get_message_opt(&self) -> Option<M> {
    self
      .underlying
      .get_message_handle_opt()
      .await
      .and_then(|handle| handle.to_typed::<M>())
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.underlying.get_message_header_handle().await
  }
}

impl<M: Message + Clone> TypedReceiverContext<M> for TypedContextHandle<M> {}

#[async_trait]
impl<M: Message> TypedReceiverPart<M> for TypedContextHandle<M> {
  async fn receive(&mut self, envelope: TypedMessageEnvelope<M>) -> Result<(), ActorError> {
    self.underlying.receive(envelope.into()).await
  }
}

impl<M: Message> TypedSpawnerContext<M> for TypedContextHandle<M> {}

#[async_trait]
impl<M: Message> TypedSpawnerPart for TypedContextHandle<M> {
  async fn spawn<A: Message + Clone>(&mut self, props: TypedProps<A>) -> TypedExtendedPid<A> {
    self.underlying.spawn(props.into()).await.into()
  }

  async fn spawn_prefix<A: Message + Clone>(&mut self, props: TypedProps<A>, prefix: &str) -> TypedExtendedPid<A> {
    self.underlying.spawn_prefix(props.into(), prefix).await.into()
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
impl<M: Message> BasePart for TypedContextHandle<M> {
  fn as_any(&self) -> &dyn Any {
    self
  }

  async fn get_receive_timeout(&self) -> Duration {
    self.underlying.get_receive_timeout().await
  }

  async fn get_children(&self) -> Vec<ExtendedPid> {
    self.underlying.get_children().await
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
impl<M: Message> TypedStopperPart<M> for TypedContextHandle<M> {
  async fn stop(&mut self, pid: &TypedExtendedPid<M>) {
    self.underlying.stop(pid.get_underlying()).await
  }

  async fn stop_future_with_timeout(&mut self, pid: &TypedExtendedPid<M>, timeout: Duration) -> ActorFuture {
    self
      .underlying
      .stop_future_with_timeout(pid.get_underlying(), timeout)
      .await
  }

  async fn poison(&mut self, pid: &TypedExtendedPid<M>) {
    self.underlying.poison(pid.get_underlying()).await
  }

  async fn poison_future_with_timeout(&mut self, pid: &TypedExtendedPid<M>, timeout: Duration) -> ActorFuture {
    self
      .underlying
      .poison_future_with_timeout(pid.get_underlying(), timeout)
      .await
  }
}

impl<M: Message + Clone> TypedContext<M> for TypedContextHandle<M> {}
