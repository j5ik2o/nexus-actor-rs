//! Mock context implementation for testing.

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
pub struct MockContext {
  actor_system: Arc<RwLock<ActorSystem>>,
  self_pid: Option<Pid>,
  parent_pid: Option<Pid>,
  message: Option<MessageHandle>,
  message_headers: Option<Arc<RwLock<dyn std::any::Any + Send + Sync>>>,
  message_envelope: Option<MessageOrEnvelope>,
  receive_timeout: Duration,
}

impl MockContext {
  pub fn new(actor_system: Arc<RwLock<ActorSystem>>) -> Self {
    Self {
      actor_system,
      self_pid: None,
      parent_pid: None,
      message: None,
      message_headers: None,
      message_envelope: None,
      receive_timeout: Duration::from_secs(0),
    }
  }
}

#[async_trait]
impl Context for MockContext {
  async fn get_self_opt(&self) -> Option<Pid> {
    self.self_pid.clone()
  }

  async fn get_self(&self) -> Pid {
    self.self_pid.clone().expect("No self PID set")
  }

  async fn get_parent_opt(&self) -> Option<Pid> {
    self.parent_pid.clone()
  }

  async fn get_parent(&self) -> Pid {
    self.parent_pid.clone().expect("No parent PID set")
  }

  async fn get_actor_system(&self) -> Arc<RwLock<ActorSystem>> {
    self.actor_system.clone()
  }
}

#[async_trait]
impl MessagePart for MockContext {
  async fn get_message(&self) -> MessageHandle {
    self.message.clone().expect("No message set")
  }

  async fn get_message_headers_opt(&self) -> Option<Arc<RwLock<dyn std::any::Any + Send + Sync>>> {
    self.message_headers.clone()
  }

  async fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope> {
    self.message_envelope.clone()
  }

  async fn get_receive_timeout(&self) -> Duration {
    self.receive_timeout
  }

  async fn set_receive_timeout(&self, duration: Duration) {
    // Mock implementation
  }

  async fn cancel_receive_timeout(&self) {
    // Mock implementation
  }
}

#[async_trait]
impl ReceiverPart for MockContext {
  async fn receive(&self, _message: MessageOrEnvelope) {
    // Mock implementation
  }
}

#[async_trait]
impl SenderPart for MockContext {
  async fn request(&self, _target: &Pid, message: MessageHandle) -> MessageHandle {
    message
  }

  async fn forward(&self, _target: &Pid, _message: MessageHandle) {
    // Mock implementation
  }
}

#[async_trait]
impl SpawnerPart for MockContext {
  async fn spawn(&self, _props: Props) -> Result<Pid, ActorError> {
    Ok(Pid::new("mock".to_string(), 0))
  }

  async fn spawn_prefix(&self, _props: Props, _prefix: &str) -> Result<Pid, ActorError> {
    Ok(Pid::new("mock".to_string(), 0))
  }
}

#[async_trait]
impl StopperPart for MockContext {
  async fn watch(&self, _pid: &Pid) {
    // Mock implementation
  }

  async fn unwatch(&self, _pid: &Pid) {
    // Mock implementation
  }

  async fn handle_failure(&self, _who: Option<Pid>, _error: ActorError, _message: Option<MessageHandle>) {
    // Mock implementation
  }
}

impl ActorContext for MockContext {}
