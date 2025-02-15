use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{ActorSystem, ErrorReason, Message, MessageHandle, Pid, Props, SpawnError};

#[async_trait]
pub trait ActorContext: Debug + Send + Sync + 'static {
  async fn get_message(&self) -> MessageHandle;
  async fn parent(&self) -> Option<Pid>;
  async fn self_pid(&self) -> Pid;
  async fn actor_system(&self) -> Arc<RwLock<ActorSystem>>;
  async fn spawn(&self, props: Props) -> Result<Pid, SpawnError>;
  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError>;
  async fn watch(&self, pid: &Pid);
  async fn unwatch(&self, pid: &Pid);
  async fn set_receive_timeout(&self, duration: std::time::Duration);
  async fn cancel_receive_timeout(&self);
  async fn forward(&self, pid: &Pid, message: MessageHandle);
  async fn forward_system(&self, pid: &Pid, message: MessageHandle);
  async fn stop(&self, pid: &Pid);
  async fn poison_pill(&self, pid: &Pid);
  async fn handle_failure(&self, who: Option<Pid>, error: ErrorReason, message: Option<MessageHandle>);
}

pub trait MessagePart {
  async fn get_message(&self) -> MessageHandle;
}

pub trait BasePart {
  fn as_context(&self) -> Option<Box<dyn ActorContext>>;
}
