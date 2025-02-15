//! Typed actor context implementation.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::system::ActorSystem;
use crate::actor::{
  ActorContext, ActorError, Context, InfoPart, Message, MessageHandle, MessageOrEnvelope, MessagePart, Pid, Props,
  ReceiverPart, SenderPart, SpawnerPart, StopperPart,
};

#[derive(Debug)]
pub struct TypedActorContext<M: Message> {
  underlying: Box<dyn ActorContext>,
  _phantom: std::marker::PhantomData<M>,
}

impl<M: Message> TypedActorContext<M> {
  pub fn new(context: Box<dyn ActorContext>) -> Self {
    Self {
      underlying: context,
      _phantom: std::marker::PhantomData,
    }
  }
}

#[async_trait]
impl<M: Message> Context for TypedActorContext<M> {
  async fn get_self_opt(&self) -> Option<Pid> {
    self.underlying.get_self_opt().await
  }

  async fn get_self(&self) -> Pid {
    self.underlying.get_self().await
  }

  async fn get_parent_opt(&self) -> Option<Pid> {
    self.underlying.get_parent_opt().await
  }

  async fn get_parent(&self) -> Pid {
    self.underlying.get_parent().await
  }

  async fn get_actor_system(&self) -> Arc<RwLock<ActorSystem>> {
    self.underlying.get_actor_system().await
  }
}

#[async_trait]
impl<M: Message> InfoPart for TypedActorContext<M> {
  async fn get_self_opt(&self) -> Option<Pid> {
    self.underlying.get_self_opt().await
  }

  async fn get_self(&self) -> Pid {
    self.underlying.get_self().await
  }

  async fn get_parent_opt(&self) -> Option<Pid> {
    self.underlying.get_parent_opt().await
  }

  async fn get_parent(&self) -> Pid {
    self.underlying.get_parent().await
  }

  async fn get_actor_system(&self) -> Arc<RwLock<ActorSystem>> {
    self.underlying.get_actor_system().await
  }
}

#[async_trait]
impl<M: Message> MessagePart for TypedActorContext<M> {
  async fn get_message(&self) -> MessageHandle {
    self.underlying.get_message().await
  }

  async fn get_message_headers_opt(&self) -> Option<Arc<RwLock<dyn std::any::Any + Send + Sync>>> {
    self.underlying.get_message_headers_opt().await
  }

  async fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope> {
    self.underlying.get_message_envelope_opt().await
  }

  async fn get_receive_timeout(&self) -> Duration {
    self.underlying.get_receive_timeout().await
  }

  async fn set_receive_timeout(&self, duration: Duration) {
    self.underlying.set_receive_timeout(duration).await
  }

  async fn cancel_receive_timeout(&self) {
    self.underlying.cancel_receive_timeout().await
  }
}

#[async_trait]
impl<M: Message> ReceiverPart for TypedActorContext<M> {
  async fn receive(&self, message: MessageOrEnvelope) {
    self.underlying.receive(message).await
  }
}

#[async_trait]
impl<M: Message> SenderPart for TypedActorContext<M> {
  async fn request(&self, target: &Pid, message: MessageHandle) -> MessageHandle {
    self.underlying.request(target, message).await
  }

  async fn forward(&self, target: &Pid, message: MessageHandle) {
    self.underlying.forward(target, message).await
  }
}

#[async_trait]
impl<M: Message> SpawnerPart for TypedActorContext<M> {
  async fn spawn(&self, props: Props) -> Result<Pid, ActorError> {
    self.underlying.spawn(props).await
  }

  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, ActorError> {
    self.underlying.spawn_prefix(props, prefix).await
  }
}

#[async_trait]
impl<M: Message> StopperPart for TypedActorContext<M> {
  async fn watch(&self, pid: &Pid) {
    self.underlying.watch(pid).await
  }

  async fn unwatch(&self, pid: &Pid) {
    self.underlying.unwatch(pid).await
  }

  async fn handle_failure(&self, who: Option<Pid>, error: ActorError, message: Option<MessageHandle>) {
    self.underlying.handle_failure(who, error, message).await
  }
}

impl<M: Message> ActorContext for TypedActorContext<M> {}
