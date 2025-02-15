use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{
  ActorContext, ActorError, ActorHandle, ActorSystem, Message, MessageHandle, Pid, Props, SpawnError,
  TypedMessageEnvelope,
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
impl<M: Message> ActorContext for TypedActorContext<M> {
  async fn get_message(&self) -> MessageHandle {
    self.underlying.get_message().await
  }

  async fn parent(&self) -> Option<Pid> {
    self.underlying.parent().await
  }

  async fn self_pid(&self) -> Pid {
    self.underlying.self_pid().await
  }

  async fn actor_system(&self) -> Arc<RwLock<ActorSystem>> {
    self.underlying.actor_system().await
  }

  async fn spawn(&self, props: Props) -> Result<Pid, SpawnError> {
    self.underlying.spawn(props).await
  }

  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError> {
    self.underlying.spawn_prefix(props, prefix).await
  }

  async fn watch(&self, pid: &Pid) {
    self.underlying.watch(pid).await
  }

  async fn unwatch(&self, pid: &Pid) {
    self.underlying.unwatch(pid).await
  }

  async fn set_receive_timeout(&self, duration: std::time::Duration) {
    self.underlying.set_receive_timeout(duration).await
  }

  async fn cancel_receive_timeout(&self) {
    self.underlying.cancel_receive_timeout().await
  }

  async fn forward(&self, pid: &Pid, message: MessageHandle) {
    self.underlying.forward(pid, message).await
  }

  async fn forward_system(&self, pid: &Pid, message: MessageHandle) {
    self.underlying.forward_system(pid, message).await
  }

  async fn stop(&self, pid: &Pid) {
    self.underlying.stop(pid).await
  }

  async fn poison_pill(&self, pid: &Pid) {
    self.underlying.poison_pill(pid).await
  }

  async fn handle_failure(&self, who: Option<Pid>, error: ActorError, message: Option<MessageHandle>) {
    self.underlying.handle_failure(who, error, message).await
  }
}
