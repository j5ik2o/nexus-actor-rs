//! Actor context module provides core actor context functionality.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::system::ActorSystem;
use crate::actor::{Message, MessageHandle, MessageOrEnvelope, Pid, Props, SpawnError};

#[async_trait]
pub trait Context: Debug + Send + Sync + 'static {
  fn as_any(&self) -> &dyn std::any::Any;
  async fn parent(&self) -> Option<Pid>;
  async fn self_pid(&self) -> Pid;
  async fn actor_system(&self) -> Arc<RwLock<dyn Debug + Send + Sync>>;
}

#[async_trait]
pub trait InfoPart: Debug + Send + Sync + 'static {
  async fn parent(&self) -> Option<Pid>;
  async fn self_pid(&self) -> Pid;
  async fn actor_system(&self) -> Arc<RwLock<dyn Debug + Send + Sync>>;
}

#[async_trait]
pub trait MessagePart: Debug + Send + Sync + 'static {
  async fn get_message(&self) -> MessageHandle;
  async fn get_message_envelope(&self) -> MessageOrEnvelope;
}

#[async_trait]
pub trait ReceiverPart: Debug + Send + Sync + 'static {
  async fn receive(&self, message: MessageOrEnvelope);
}

#[async_trait]
pub trait SenderPart: Debug + Send + Sync + 'static {
  async fn send(&self, target: &Pid, message: MessageHandle);
  async fn request(&self, target: &Pid, message: MessageHandle) -> MessageHandle;
}

#[async_trait]
pub trait SpawnerPart: Debug + Send + Sync + 'static {
  async fn spawn(&self, props: Props) -> Result<Pid, SpawnError>;
  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError>;
}

#[async_trait]
pub trait StopperPart: Debug + Send + Sync + 'static {
  async fn stop(&self, pid: &Pid);
  async fn poison_pill(&self, pid: &Pid);
}

pub trait ActorContext:
  Context + InfoPart + MessagePart + ReceiverPart + SenderPart + SpawnerPart + StopperPart {
}

// Implement ActorContext for any type that implements all required traits
impl<T> ActorContext for T where
  T: Context + InfoPart + MessagePart + ReceiverPart + SenderPart + SpawnerPart + StopperPart
{
}
