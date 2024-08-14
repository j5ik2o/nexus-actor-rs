use crate::actor::actor::{ActorHandle, ExtendedPid, Props, SpawnError};
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{
  InfoPart, MessagePart, RootContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart, TypedMessagePart,
  TypedSenderContext, TypedSenderPart,
};
use crate::actor::future::ActorFuture;
use crate::actor::message::{Message, MessageHandle, ReadonlyMessageHeadersHandle, TypedMessageEnvelope};
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

#[async_trait]
impl<T: Message + Clone> TypedMessagePart<T> for TypedRootContext<T> {
  async fn get_message_envelope_opt(&self) -> Option<TypedMessageEnvelope<T>> {
    self
      .inner
      .get_message_envelope_opt()
      .await
      .map(|e| TypedMessageEnvelope::new(e))
  }

  async fn get_message_opt(&self) -> Option<T> {
    self
      .inner
      .get_message_handle_opt()
      .await
      .and_then(|h| h.to_typed::<T>())
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.inner.get_message_header_handle().await
  }
}

#[async_trait]
impl<T: Message> TypedSenderPart<T> for TypedRootContext<T> {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    self.inner.get_sender().await
  }

  async fn send(&mut self, pid: ExtendedPid, message: T) {
    self.inner.send(pid, MessageHandle::new(message)).await
  }

  async fn request(&mut self, pid: ExtendedPid, message: T) {
    self.inner.request(pid, MessageHandle::new(message)).await
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message: T, sender: ExtendedPid) {
    self
      .inner
      .request_with_custom_sender(pid, MessageHandle::new(message), sender)
      .await
  }

  async fn request_future(&self, pid: ExtendedPid, message: T, timeout: Duration) -> ActorFuture {
    self
      .inner
      .request_future(pid, MessageHandle::new(message), timeout)
      .await
  }
}

impl<T: Message + Clone> TypedSenderContext<T> for TypedRootContext<T> {}

#[async_trait]
impl<T: Message> SpawnerPart for TypedRootContext<T> {
  async fn spawn(&mut self, props: Props) -> ExtendedPid {
    self.inner.spawn(props).await
  }

  async fn spawn_prefix(&mut self, props: Props, prefix: &str) -> ExtendedPid {
    self.inner.spawn_prefix(props, prefix).await
  }

  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<ExtendedPid, SpawnError> {
    self.inner.spawn_named(props, id).await
  }
}
#[async_trait]
impl<T: Message> InfoPart for TypedRootContext<T> {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    self.inner.get_parent().await
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
    self.inner.get_self_opt().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    self.inner.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    self.inner.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.inner.get_actor_system().await
  }
}

impl<T: Message> SpawnerContext for TypedRootContext<T> {}

#[async_trait]
impl<T: Message> StopperPart for TypedRootContext<T> {
  async fn stop(&mut self, pid: &ExtendedPid) {
    self.inner.stop(pid).await
  }

  async fn stop_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    self.inner.stop_future_with_timeout(pid, timeout).await
  }

  async fn poison(&mut self, pid: &ExtendedPid) {
    self.inner.poison(pid).await
  }

  async fn poison_future_with_timeout(&mut self, pid: &ExtendedPid, timeout: Duration) -> ActorFuture {
    self.inner.poison_future_with_timeout(pid, timeout).await
  }
}
