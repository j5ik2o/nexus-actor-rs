use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::{
  ActorSystem, Context, InfoPart, Message, MessageHandle, MessageOrEnvelope, MessagePart, Pid, Props, SpawnError,
  SpawnerContext, SpawnerPart,
};

#[derive(Debug)]
pub struct SpawnerContextHandle {
  inner: Arc<RwLock<ActorSystem>>,
}

impl SpawnerContextHandle {
  pub fn new(actor_system: Arc<RwLock<ActorSystem>>) -> Self {
    Self { inner: actor_system }
  }
}

#[async_trait]
impl Context for SpawnerContextHandle {
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
    self.inner.clone()
  }
}

#[async_trait]
impl InfoPart for SpawnerContextHandle {
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
    self.inner.clone()
  }
}

#[async_trait]
impl MessagePart for SpawnerContextHandle {
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
impl SpawnerPart for SpawnerContextHandle {
  async fn spawn(&self, props: Props) -> Result<Pid, SpawnError> {
    self.inner.read().await.spawn(props).await
  }

  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError> {
    self.inner.read().await.spawn_prefix(props, prefix).await
  }
}

impl SpawnerContext for SpawnerContextHandle {}
