use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::{
  ActorContext, ActorError, ActorHandle, ActorSystem, Message, MessageHandle, MessageOrEnvelope, Pid, Props,
  ReadonlyMessageHeadersHandle, ResponseHandle, SpawnError,
};

#[derive(Debug, Clone)]
pub struct RootContext {
  actor_system: ActorSystem,
  guardian_strategy: Option<Arc<RwLock<dyn ActorContext>>>,
}

impl Context for RootContext {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[async_trait]
impl InfoPart for RootContext {
  async fn get_children(&self) -> Vec<Pid> {
    self
      .guardian_strategy
      .as_ref()
      .map_or_else(Vec::new, |g| g.read().await.get_children().await)
  }

  async fn get_receive_timeout(&self) -> Duration {
    Duration::from_secs(0)
  }

  async fn get_parent(&self) -> Option<Pid> {
    None
  }

  async fn get_self_opt(&self) -> Option<Pid> {
    None
  }

  async fn set_self(&mut self, _pid: Pid) {}

  async fn get_actor(&self) -> Option<ActorHandle> {
    None
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.actor_system.clone()
  }
}

#[async_trait]
impl MessagePart for RootContext {
  async fn get_message(&self) -> MessageHandle {
    unimplemented!("Root context does not handle messages")
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    None
  }

  async fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope> {
    None
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    None
  }
}

#[async_trait]
impl SenderPart for RootContext {
  async fn forward(&self, _pid: &Pid) {}

  async fn respond(&self, _response: ResponseHandle) {}

  async fn get_sender(&self) -> Option<Pid> {
    None
  }

  async fn send(&mut self, pid: Pid, message_handle: MessageHandle) {
    self.actor_system.send_user_message(pid, message_handle).await;
  }

  async fn request(&mut self, pid: Pid, message_handle: MessageHandle) {
    self.send(pid, message_handle).await;
  }

  async fn request_with_custom_sender(&mut self, pid: Pid, message_handle: MessageHandle, _sender: Pid) {
    self.send(pid, message_handle).await;
  }

  async fn request_future(&self, pid: Pid, message_handle: MessageHandle) -> Result<ResponseHandle, ActorError> {
    self.actor_system.request_future(pid, message_handle).await
  }
}

#[async_trait]
impl SpawnerPart for RootContext {
  async fn spawn(&self, props: Props) -> Result<Pid, SpawnError> {
    self.actor_system.spawn(props).await
  }

  async fn spawn_prefix(&self, props: Props, prefix: &str) -> Result<Pid, SpawnError> {
    self.actor_system.spawn_prefix(props, prefix).await
  }

  async fn spawn_named(&mut self, props: Props, id: &str) -> Result<Pid, SpawnError> {
    self.actor_system.spawn_named(props, id).await
  }
}

#[async_trait]
impl StopperPart for RootContext {
  async fn stop(&self) {}

  async fn poison_pill(&self) {}
}

impl RootContext {
  pub fn new(actor_system: ActorSystem) -> Self {
    Self {
      actor_system,
      guardian_strategy: None,
    }
  }

  pub fn with_guardian(mut self, guardian: Arc<RwLock<dyn ActorContext>>) -> Self {
    self.guardian_strategy = Some(guardian);
    self
  }

  pub fn to_typed<M: Message>(&self) -> TypedRootContext<M> {
    TypedRootContext::new(self.clone())
  }
}
