use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::{ActorError, ActorSystem, Message, MessageHandle, MessageOrEnvelope, Pid, Props, SpawnError};

#[async_trait]
pub trait Context: Debug + Send + Sync + 'static {
  fn as_any(&self) -> &dyn Any;
  async fn parent(&self) -> Option<Pid>;
  async fn self_pid(&self) -> Pid;
  async fn actor_system(&self) -> Arc<RwLock<ActorSystem>>;
}

#[async_trait]
pub trait InfoPart: Debug + Send + Sync + 'static {
  async fn get_self_opt(&self) -> Option<Pid>;
  async fn get_self(&self) -> Pid;
  async fn get_parent_opt(&self) -> Option<Pid>;
  async fn get_parent(&self) -> Pid;
  async fn get_actor_system(&self) -> Arc<RwLock<ActorSystem>>;
}

#[async_trait]
pub trait MessagePart: Debug + Send + Sync + 'static {
  async fn get_message(&self) -> MessageHandle;
  async fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope>;
  async fn get_message_envelope(&self) -> MessageOrEnvelope;
  async fn get_receive_timeout(&self) -> Duration;
  async fn set_receive_timeout(&self, duration: Duration);
  async fn cancel_receive_timeout(&self);
}

#[async_trait]
pub trait ReceiverPart: Debug + Send + Sync + 'static {
  async fn receive(&self, message: MessageOrEnvelope);
}

#[async_trait]
pub trait SenderPart: Debug + Send + Sync + 'static {
  async fn send(&self, target: &Pid, message: MessageHandle);
  async fn request(&self, target: &Pid, message: MessageHandle) -> MessageHandle;
  async fn forward(&self, target: &Pid, message: MessageHandle);
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
  async fn watch(&self, pid: &Pid);
  async fn unwatch(&self, pid: &Pid);
  async fn handle_failure(&self, who: Option<Pid>, error: ActorError, message: Option<MessageHandle>);
}

pub trait ActorContext:
  Context + InfoPart + MessagePart + ReceiverPart + SenderPart + SpawnerPart + StopperPart {
}
pub trait ReceiverContext: Context + InfoPart + MessagePart + ReceiverPart {}
pub trait SenderContext: Context + InfoPart + SenderPart {}
pub trait SpawnerContext: Context + InfoPart + SpawnerPart {}
pub trait RootContext: Context + InfoPart + SenderPart + SpawnerPart + StopperPart {}
