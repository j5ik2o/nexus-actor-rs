use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::{
  ActorSystem, Context, ExtensionPart, InfoPart, Message, MessageHandle, MessageOrEnvelope, MessagePart, Pid,
  ReadonlyMessageHeadersHandle, TypedContext,
};

#[async_trait]
impl<M: Message> Context for TypedContextHandle<M> {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[async_trait]
impl<M: Message> InfoPart for TypedContextHandle<M> {
  async fn get_self_opt(&self) -> Option<Pid> {
    self.inner.read().await.get_self_opt().await
  }

  async fn get_self(&self) -> Pid {
    self.inner.read().await.get_self().await
  }

  async fn get_parent_opt(&self) -> Option<Pid> {
    self.inner.read().await.get_parent_opt().await
  }

  async fn get_parent(&self) -> Pid {
    self.inner.read().await.get_parent().await
  }

  async fn get_actor_system(&self) -> Arc<RwLock<ActorSystem>> {
    self.inner.read().await.get_actor_system().await
  }
}

#[async_trait]
impl<M: Message> MessagePart for TypedContextHandle<M> {
  async fn get_message_headers_opt(&self) -> Option<Arc<RwLock<dyn Any + Send + Sync>>> {
    self.inner.read().await.get_message_headers_opt().await
  }

  async fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope> {
    self.inner.read().await.get_message_envelope_opt().await
  }

  async fn get_message_envelope(&self) -> MessageOrEnvelope {
    self.inner.read().await.get_message_envelope().await
  }

  async fn get_receive_timeout(&self) -> Duration {
    self.inner.read().await.get_receive_timeout().await
  }

  async fn set_receive_timeout(&self, duration: Duration) {
    self.inner.read().await.set_receive_timeout(duration).await
  }

  async fn cancel_receive_timeout(&self) {
    self.inner.read().await.cancel_receive_timeout().await
  }
}

#[async_trait]
impl<M: Message> ExtensionPart for TypedContextHandle<M> {
  async fn get_extension<T: Any + Send + Sync>(&self) -> Option<Arc<RwLock<T>>> {
    self.inner.read().await.get_extension().await
  }

  async fn set_extension<T: Any + Send + Sync>(&self, extension: T) {
    self.inner.read().await.set_extension(extension).await
  }
}

#[async_trait]
impl<M: Message + Clone> TypedContext<M> for TypedContextHandle<M> {
  async fn get_message(&self) -> M {
    self.inner.read().await.get_message().await
  }

  async fn get_message_envelope_opt(&self) -> Option<TypedMessageEnvelope<M>> {
    self.inner.read().await.get_message_envelope_opt().await
  }

  async fn get_message_envelope(&self) -> TypedMessageEnvelope<M> {
    self.inner.read().await.get_message_envelope().await
  }
}

pub struct TypedContextHandle<M: Message> {
  inner: Arc<RwLock<dyn TypedContext<M> + Send + Sync>>,
}

impl<M: Message> TypedContextHandle<M> {
  pub fn new(inner: Arc<RwLock<dyn TypedContext<M> + Send + Sync>>) -> Self {
    Self { inner }
  }
}

impl<M: Message> Debug for TypedContextHandle<M> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TypedContextHandle").finish()
  }
}
