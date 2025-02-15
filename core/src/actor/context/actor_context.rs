use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::{ActorError, ErrorReason, Message, MessageHandle, MessageOrEnvelope, Pid, Props, SpawnError};

pub struct ActorSystem {
  inner: Arc<RwLock<ActorSystemInner>>,
}

struct ActorSystemInner {
  address: String,
}

#[async_trait]
pub trait Context: Debug + Send + Sync + 'static {
  fn as_any(&self) -> &dyn Any;
}

#[async_trait]
pub trait InfoPart: Context {
  async fn get_self_opt(&self) -> Option<Pid>;
  async fn get_self(&self) -> Pid;
  async fn get_parent_opt(&self) -> Option<Pid>;
  async fn get_parent(&self) -> Pid;
  async fn get_actor_system(&self) -> Arc<RwLock<ActorSystem>>;
}

#[async_trait]
pub trait MessagePart: Context {
  async fn get_message_headers_opt(&self) -> Option<Arc<RwLock<dyn Any + Send + Sync>>>;
  async fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope>;
  async fn get_message_envelope(&self) -> MessageOrEnvelope;
  async fn get_receive_timeout(&self) -> Duration;
  async fn set_receive_timeout(&self, duration: Duration);
  async fn cancel_receive_timeout(&self);
}

#[async_trait]
pub trait SenderPart: Context {
  async fn send(&self, target: &Pid, message: MessageHandle);
  async fn request(&self, target: &Pid, message: MessageHandle) -> Result<Box<dyn Message>, ActorError>;
  async fn forward(&self, target: &Pid, message: MessageHandle);
}

#[async_trait]
pub trait ReceiverPart: Context {
  async fn receive(&self, message: Box<dyn Message>);
}

#[async_trait]
pub trait SpawnerPart: Context {
  async fn spawn(&self, props: Props) -> Result<Pid, SpawnError>;
  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError>;
  async fn spawn_named(&self, props: Props, name: &str) -> Result<Pid, SpawnError>;
}

#[async_trait]
pub trait StopperPart: Context {
  async fn stop(&self, pid: &Pid);
  async fn poison_pill(&self, pid: &Pid);
  async fn watch(&self, pid: &Pid);
  async fn unwatch(&self, pid: &Pid);
  async fn handle_failure(&self, who: Option<Pid>, error: ErrorReason, message: Option<MessageHandle>);
}

pub trait SenderContext: Context + InfoPart + MessagePart + SenderPart {}
pub trait ReceiverContext: Context + InfoPart + MessagePart + ReceiverPart {}
pub trait SpawnerContext: Context + InfoPart + MessagePart + SpawnerPart {}
pub trait TypedContext<M: Message>: Context + InfoPart + MessagePart {}
pub trait RootContext: Context + InfoPart + MessagePart + SenderPart + SpawnerPart + StopperPart {}
pub trait TypedRootContext<M: Message>: RootContext + TypedContext<M> {}

impl ActorSystem {
  pub fn new(address: String) -> Self {
    Self {
      inner: Arc::new(RwLock::new(ActorSystemInner { address })),
    }
  }

  pub async fn address(&self) -> String {
    self.inner.read().await.address.clone()
  }
}
