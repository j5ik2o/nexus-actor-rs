use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{
  ActorContext, ActorError, ActorHandle, ActorSystem, Message, MessageHandle, Pid, Props, SpawnError,
  TypedMessageEnvelope,
};

#[derive(Debug)]
pub struct TypedContextHandle<M: Message> {
  underlying: Arc<RwLock<dyn ActorContext>>,
  _phantom: std::marker::PhantomData<M>,
}

impl<M: Message> Context for TypedContextHandle<M> {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[async_trait]
impl<M: Message> InfoPart for TypedContextHandle<M> {
  async fn get_children(&self) -> Vec<Pid> {
    self.underlying.read().await.get_children().await
  }

  async fn get_receive_timeout(&self) -> Duration {
    self.underlying.read().await.get_receive_timeout().await
  }

  async fn get_parent(&self) -> Option<Pid> {
    self.underlying.read().await.get_parent().await
  }

  async fn get_self_opt(&self) -> Option<Pid> {
    self.underlying.read().await.get_self_opt().await
  }

  async fn set_self(&mut self, pid: Pid) {
    self.underlying.write().await.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    self.underlying.read().await.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.underlying.read().await.get_actor_system().await
  }
}

#[async_trait]
impl<M: Message> MessagePart for TypedContextHandle<M> {
  async fn get_message(&self) -> MessageHandle {
    self.underlying.read().await.get_message().await
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.underlying.read().await.get_message_header_handle().await
  }

  async fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope> {
    self.underlying.read().await.get_message_envelope_opt().await
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    self.underlying.read().await.get_message_handle_opt().await
  }
}

#[async_trait]
impl<M: Message> ExtensionPart for TypedContextHandle<M> {
  async fn register_extension<T: 'static>(&mut self, extension: T) {
    self.underlying.write().await.register_extension(extension).await
  }

  async fn get_extension<T: 'static>(&self) -> Option<&T> {
    self.underlying.read().await.get_extension::<T>().await
  }

  async fn get_extension_mut<T: 'static>(&mut self) -> Option<&mut T> {
    self.underlying.write().await.get_extension_mut::<T>().await
  }
}

impl<M: Message + Clone> TypedContext<M> for TypedContextHandle<M> {
  async fn get_message(&self) -> M {
    let msg = self.underlying.read().await.get_message().await;
    msg.downcast::<M>().expect("Invalid message type")
  }

  async fn get_message_envelope_opt(&self) -> Option<TypedMessageEnvelope<M>> {
    let env = self.underlying.read().await.get_message_envelope_opt().await?;
    Some(TypedMessageEnvelope::new(
      env.message.downcast::<M>().expect("Invalid message type"),
      env.header,
      env.sender,
    ))
  }

  async fn get_message_envelope(&self) -> TypedMessageEnvelope<M> {
    self.get_message_envelope_opt().await.expect("No message envelope")
  }

  async fn parent(&self) -> Option<Pid> {
    self.get_parent().await
  }

  async fn self_pid(&self) -> Pid {
    self.get_self_opt().await.expect("No self pid")
  }

  async fn actor_system(&self) -> Arc<RwLock<ActorSystem>> {
    Arc::new(RwLock::new(self.get_actor_system().await))
  }

  async fn spawn(&self, props: Props) -> Result<Pid, SpawnError> {
    self.underlying.read().await.spawn(props).await
  }

  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError> {
    self.underlying.read().await.spawn_prefix(props, prefix).await
  }

  async fn watch(&self, pid: &Pid) {
    self.underlying.read().await.watch(pid).await
  }

  async fn unwatch(&self, pid: &Pid) {
    self.underlying.read().await.unwatch(pid).await
  }

  async fn set_receive_timeout(&self, duration: Duration) {
    self.underlying.read().await.set_receive_timeout(&duration).await
  }

  async fn cancel_receive_timeout(&self) {
    self.underlying.read().await.cancel_receive_timeout().await
  }

  async fn forward(&self, pid: &Pid, message: MessageHandle) {
    self.underlying.read().await.forward(pid, message).await
  }

  async fn forward_system(&self, pid: &Pid, message: MessageHandle) {
    self.underlying.read().await.forward_system(pid, message).await
  }

  async fn stop(&self, pid: &Pid) {
    self.underlying.read().await.stop(pid).await
  }

  async fn poison_pill(&self, pid: &Pid) {
    self.underlying.read().await.poison_pill(pid).await
  }

  async fn handle_failure(&self, who: Option<Pid>, error: ActorError, message: Option<MessageHandle>) {
    self.underlying.read().await.handle_failure(who, error, message).await
  }
}
