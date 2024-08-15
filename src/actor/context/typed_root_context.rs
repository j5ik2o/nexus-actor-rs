use crate::actor::actor::{ActorHandle, SpawnError, TypedExtendedPid, TypedProps};
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{InfoPart, MessagePart, RootContext, SenderPart, SpawnerPart, StopperPart};
use crate::actor::future::ActorFuture;
use crate::actor::message::{Message, MessageHandle, ReadonlyMessageHeadersHandle, TypedMessageEnvelope};
use crate::actor::typed_context::{
  TypedInfoPart, TypedMessagePart, TypedSenderContext, TypedSenderPart, TypedSpawnerContext, TypedSpawnerPart,
  TypedStopperPart,
};
use async_trait::async_trait;
use std::marker::PhantomData;
use std::time::Duration;

#[derive(Debug)]
pub struct TypedRootContext<T> {
  inner: RootContext,
  _phantom: PhantomData<T>,
}

impl<T: Message> TypedRootContext<T> {
  pub fn new(inner: RootContext) -> Self {
    Self {
      inner,
      _phantom: PhantomData::default(),
    }
  }
}

impl<M: Message + Clone> TypedSenderContext<M> for TypedRootContext<M> {}

#[async_trait]
impl<M: Message> TypedInfoPart<M> for TypedRootContext<M> {
  async fn get_parent(&self) -> Option<TypedExtendedPid<M>> {
    self.inner.get_parent().await.map(|pid| pid.into())
  }

  async fn get_self_opt(&self) -> Option<TypedExtendedPid<M>> {
    self.inner.get_self_opt().await.map(|pid| pid.into())
  }

  async fn set_self(&mut self, pid: TypedExtendedPid<M>) {
    self.inner.set_self(pid.into()).await;
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    self.inner.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.inner.get_actor_system().await
  }
}

#[async_trait]
impl<M: Message> TypedSenderPart<M> for TypedRootContext<M> {
  async fn get_sender(&self) -> Option<TypedExtendedPid<M>> {
    self.inner.get_sender().await.map(|pid| TypedExtendedPid::new(pid))
  }

  async fn send<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A) {
    self
      .inner
      .send(pid.get_underlying().clone(), MessageHandle::new(message))
      .await;
  }

  async fn request<A: Message>(&mut self, pid: TypedExtendedPid<A>, message: A) {
    self
      .inner
      .request(pid.get_underlying().clone(), MessageHandle::new(message))
      .await;
  }

  async fn request_with_custom_sender<A: Message, B: Message>(
    &mut self,
    pid: TypedExtendedPid<A>,
    message: A,
    sender: TypedExtendedPid<B>,
  ) {
    self
      .inner
      .request_with_custom_sender(
        pid.get_underlying().clone(),
        MessageHandle::new(message),
        sender.get_underlying().clone(),
      )
      .await;
  }

  async fn request_future<A: Message>(&self, pid: TypedExtendedPid<A>, message: A, timeout: Duration) -> ActorFuture {
    self
      .inner
      .request_future(pid.get_underlying().clone(), MessageHandle::new(message), timeout)
      .await
  }
}

#[async_trait]
impl<M: Message + Clone> TypedMessagePart<M> for TypedRootContext<M> {
  async fn get_message_envelope_opt(&self) -> Option<TypedMessageEnvelope<M>> {
    self
      .inner
      .get_message_envelope_opt()
      .await
      .map(|envelope| TypedMessageEnvelope::new(envelope))
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    self.inner.get_message_handle_opt().await
  }

  async fn get_message_opt(&self) -> Option<M> {
    self
      .inner
      .get_message_handle_opt()
      .await
      .and_then(|handle| handle.to_typed::<M>())
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.inner.get_message_header_handle().await
  }
}

impl<M: Message + Clone> TypedSpawnerContext<M> for TypedRootContext<M> {}

#[async_trait]
impl<M: Message + Clone> TypedSpawnerPart for TypedRootContext<M> {
  async fn spawn<A: Message + Clone>(&mut self, props: TypedProps<A>) -> TypedExtendedPid<A> {
    TypedExtendedPid::new(self.inner.spawn(props.get_underlying().clone()).await)
  }

  async fn spawn_prefix<A: Message + Clone>(&mut self, props: TypedProps<A>, prefix: &str) -> TypedExtendedPid<A> {
    TypedExtendedPid::new(self.inner.spawn_prefix(props.get_underlying().clone(), prefix).await)
  }

  async fn spawn_named<A: Message + Clone>(
    &mut self,
    props: TypedProps<A>,
    id: &str,
  ) -> Result<TypedExtendedPid<A>, SpawnError> {
    self
      .inner
      .spawn_named(props.get_underlying().clone(), id)
      .await
      .map(|pid| TypedExtendedPid::new(pid))
  }
}

#[async_trait]
impl<M: Message> TypedStopperPart<M> for TypedRootContext<M> {
  async fn stop(&mut self, pid: &TypedExtendedPid<M>) {
    self.inner.stop(pid.get_underlying()).await;
  }

  async fn stop_future_with_timeout(&mut self, pid: &TypedExtendedPid<M>, timeout: Duration) -> ActorFuture {
    self.inner.stop_future_with_timeout(pid.get_underlying(), timeout).await
  }

  async fn poison(&mut self, pid: &TypedExtendedPid<M>) {
    self.inner.poison(pid.get_underlying()).await;
  }

  async fn poison_future_with_timeout(&mut self, pid: &TypedExtendedPid<M>, timeout: Duration) -> ActorFuture {
    self
      .inner
      .poison_future_with_timeout(pid.get_underlying(), timeout)
      .await
  }
}
