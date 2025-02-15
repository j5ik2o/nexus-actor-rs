//! Actor context trait definitions.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::actor_error::ActorError;
use crate::actor::message::{MessageHandle, MessageOrEnvelope};
use crate::actor::pid::Pid;
use crate::actor::props::Props;
use crate::actor::system::ActorSystem;
use std::time::Duration;

// Base trait for all context types
#[async_trait]
pub trait Context: Debug + Send + Sync + 'static {
  async fn get_self_opt(&self) -> Option<Pid>;
  async fn get_self(&self) -> Pid;
  async fn get_parent_opt(&self) -> Option<Pid>;
  async fn get_parent(&self) -> Pid;
  async fn get_actor_system(&self) -> Arc<RwLock<ActorSystem>>;
}

// Trait for message handling
#[async_trait]
pub trait MessagePart: Context {
  async fn get_message(&self) -> MessageHandle;
  async fn get_message_headers_opt(&self) -> Option<Arc<RwLock<dyn std::any::Any + Send + Sync>>>;
  async fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope>;
  async fn get_receive_timeout(&self) -> Duration;
  async fn set_receive_timeout(&self, duration: Duration);
  async fn cancel_receive_timeout(&self);
}

// Trait for receiving messages
#[async_trait]
pub trait ReceiverPart: Context {
  async fn receive(&self, message: MessageOrEnvelope);
}

// Trait for sending messages
#[async_trait]
pub trait SenderPart: Context {
  async fn request(&self, target: &Pid, message: MessageHandle) -> MessageHandle;
  async fn forward(&self, target: &Pid, message: MessageHandle);
}

// Trait for spawning actors
#[async_trait]
pub trait SpawnerPart: Context {
  async fn spawn(&self, props: Props) -> Result<Pid, ActorError>;
  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, ActorError>;
}

// Trait for stopping actors
#[async_trait]
pub trait StopperPart: Context {
  async fn watch(&self, pid: &Pid);
  async fn unwatch(&self, pid: &Pid);
  async fn handle_failure(&self, who: Option<Pid>, error: ActorError, message: Option<MessageHandle>);
}

// Main actor context trait combining all functionality
pub trait ActorContext: Context + MessagePart + ReceiverPart + SenderPart + SpawnerPart + StopperPart {}

// Create new traits that combine specific functionality
pub trait ReceiverContext: Context + MessagePart + ReceiverPart {}
pub trait SenderContext: Context + SenderPart {}
pub trait SpawnerContext: Context + SpawnerPart {}
pub trait RootContext: Context + SenderPart + SpawnerPart + StopperPart {}

// Implement blanket implementations
impl<T> ActorContext for T where T: Context + MessagePart + ReceiverPart + SenderPart + SpawnerPart + StopperPart {}
impl<T> ReceiverContext for T where T: Context + MessagePart + ReceiverPart {}
impl<T> SenderContext for T where T: Context + SenderPart {}
impl<T> SpawnerContext for T where T: Context + SpawnerPart {}
impl<T> RootContext for T where T: Context + SenderPart + SpawnerPart + StopperPart {}
