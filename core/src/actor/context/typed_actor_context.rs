//! Typed actor context implementation.

use async_trait::async_trait;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{
  ActorContext, Context, InfoPart, Message, MessageHandle, MessageOrEnvelope, MessagePart, Pid, Props, ReceiverPart,
  SenderPart, SpawnerPart, StopperPart,
};

#[derive(Debug)]
pub struct TypedActorContext<M: Message> {
  underlying: Box<dyn ActorContext>,
  _phantom: PhantomData<M>,
}

impl<M: Message> TypedActorContext<M> {
  pub fn new(context: Box<dyn ActorContext>) -> Self {
    Self {
      underlying: context,
      _phantom: PhantomData,
    }
  }
}

#[async_trait]
impl<M: Message> Context for TypedActorContext<M> {
  fn as_any(&self) -> &dyn Any {
    self
  }

  async fn parent(&self) -> Option<Pid> {
    self.underlying.parent().await
  }

  async fn self_pid(&self) -> Pid {
    self.underlying.self_pid().await
  }

  async fn actor_system(&self) -> Arc<RwLock<dyn Debug + Send + Sync>> {
    self.underlying.actor_system().await
  }
}

#[async_trait]
impl<M: Message> InfoPart for TypedActorContext<M> {
  async fn parent(&self) -> Option<Pid> {
    self.underlying.parent().await
  }

  async fn self_pid(&self) -> Pid {
    self.underlying.self_pid().await
  }

  async fn actor_system(&self) -> Arc<RwLock<dyn Debug + Send + Sync>> {
    self.underlying.actor_system().await
  }
}

#[async_trait]
impl<M: Message> MessagePart for TypedActorContext<M> {
  async fn get_message(&self) -> MessageHandle {
    self.underlying.get_message().await
  }

  async fn get_message_envelope(&self) -> MessageOrEnvelope {
    self.underlying.get_message_envelope().await
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
  async fn send(&self, target: &Pid, message: MessageHandle) {
    self.underlying.send(target, message).await
  }

  async fn request(&self, target: &Pid, message: MessageHandle) -> MessageHandle {
    self.underlying.request(target, message).await
  }
}

#[async_trait]
impl<M: Message> SpawnerPart for TypedActorContext<M> {
  async fn spawn(&self, props: Props) -> Result<Pid, SpawnError> {
    self.underlying.spawn(props).await
  }

  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError> {
    self.underlying.spawn_prefix(props, prefix).await
  }
}

#[async_trait]
impl<M: Message> StopperPart for TypedActorContext<M> {
  async fn stop(&self, pid: &Pid) {
    self.underlying.stop(pid).await
  }

  async fn poison_pill(&self, pid: &Pid) {
    self.underlying.poison_pill(pid).await
  }
}

impl<M: Message> ActorContext for TypedActorContext<M> {}
