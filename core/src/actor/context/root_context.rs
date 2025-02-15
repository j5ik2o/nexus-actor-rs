use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::{
  ActorError, ActorSystem, Context, InfoPart, Message, MessageHandle, MessageOrEnvelope, MessagePart, Pid, Props,
  ReceiverPart, SenderPart, SpawnError, SpawnerPart, StopperPart,
};

#[derive(Debug)]
pub struct RootContext {
  pub(crate) actor_system: Arc<RwLock<ActorSystem>>,
}

impl RootContext {
  pub fn new(actor_system: Arc<RwLock<ActorSystem>>) -> Self {
    Self { actor_system }
  }
}

#[async_trait]
impl Context for RootContext {
  fn as_any(&self) -> &dyn Any {
    self
  }

  async fn parent(&self) -> Option<Pid> {
    None
  }

  async fn self_pid(&self) -> Pid {
    unimplemented!()
  }

  async fn actor_system(&self) -> Arc<RwLock<ActorSystem>> {
    self.actor_system.clone()
  }
}

#[async_trait]
impl InfoPart for RootContext {
  async fn get_self_opt(&self) -> Option<Pid> {
    None
  }

  async fn get_self(&self) -> Pid {
    unimplemented!()
  }

  async fn get_parent_opt(&self) -> Option<Pid> {
    None
  }

  async fn get_parent(&self) -> Pid {
    unimplemented!()
  }

  async fn get_actor_system(&self) -> Arc<RwLock<ActorSystem>> {
    self.actor_system.clone()
  }
}

#[async_trait]
impl MessagePart for RootContext {
  async fn get_message(&self) -> MessageHandle {
    unimplemented!()
  }

  async fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope> {
    None
  }

  async fn get_message_envelope(&self) -> MessageOrEnvelope {
    unimplemented!()
  }

  async fn get_receive_timeout(&self) -> Duration {
    Duration::from_secs(0)
  }

  async fn set_receive_timeout(&self, _duration: Duration) {}

  async fn cancel_receive_timeout(&self) {}
}

#[async_trait]
impl SenderPart for RootContext {
  async fn send(&self, target: &Pid, message: MessageHandle) {
    self.actor_system.read().await.send(target, message).await;
  }

  async fn request(&self, target: &Pid, message: MessageHandle) -> MessageHandle {
    self.actor_system.read().await.request(target, message).await
  }

  async fn forward(&self, target: &Pid, message: MessageHandle) {
    self.actor_system.read().await.forward(target, message).await;
  }
}

#[async_trait]
impl SpawnerPart for RootContext {
  async fn spawn(&self, props: Props) -> Result<Pid, SpawnError> {
    self.actor_system.read().await.spawn(props).await
  }

  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError> {
    self.actor_system.read().await.spawn_prefix(props, prefix).await
  }
}

#[async_trait]
impl StopperPart for RootContext {
  async fn stop(&self, pid: &Pid) {
    self.actor_system.read().await.stop(pid).await;
  }

  async fn poison_pill(&self, pid: &Pid) {
    self.actor_system.read().await.poison_pill(pid).await;
  }

  async fn watch(&self, pid: &Pid) {
    self.actor_system.read().await.watch(pid).await;
  }

  async fn unwatch(&self, pid: &Pid) {
    self.actor_system.read().await.unwatch(pid).await;
  }

  async fn handle_failure(&self, who: Option<Pid>, error: ActorError, message: Option<MessageHandle>) {
    self.actor_system.read().await.handle_failure(who, error, message).await;
  }
}
