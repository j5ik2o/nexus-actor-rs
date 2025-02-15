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

#[derive(Debug)]
pub struct MockContext {
  actor_system: ActorSystem,
  extensions: Arc<RwLock<Vec<Box<dyn Any + Send + Sync>>>>,
}

impl Context for MockContext {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[async_trait]
impl InfoPart for MockContext {
  async fn get_children(&self) -> Vec<Pid> {
    Vec::new()
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
impl MessagePart for MockContext {
  async fn get_message(&self) -> MessageHandle {
    unimplemented!("Mock context does not handle messages")
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
impl SenderPart for MockContext {
  async fn forward(&self, _pid: &Pid) {}

  async fn respond(&self, _response: ResponseHandle) {}

  async fn get_sender(&self) -> Option<Pid> {
    None
  }

  async fn send(&mut self, _pid: Pid, _message_handle: MessageHandle) {}

  async fn request(&mut self, _pid: Pid, _message_handle: MessageHandle) {}

  async fn request_with_custom_sender(&mut self, _pid: Pid, _message_handle: MessageHandle, _sender: Pid) {}

  async fn request_future(&self, _pid: Pid, _message_handle: MessageHandle) -> Result<ResponseHandle, ActorError> {
    Err(ActorError::Unimplemented)
  }
}

#[async_trait]
impl ExtensionPart for MockContext {
  async fn register_extension<T: 'static>(&mut self, extension: T) {
    self.extensions.write().await.push(Box::new(extension));
  }

  async fn get_extension<T: 'static>(&self) -> Option<&T> {
    for ext in self.extensions.read().await.iter() {
      if let Some(ext) = ext.as_any().downcast_ref::<T>() {
        return Some(ext);
      }
    }
    None
  }

  async fn get_extension_mut<T: 'static>(&mut self) -> Option<&mut T> {
    for ext in self.extensions.write().await.iter_mut() {
      if let Some(ext) = ext.as_any_mut().downcast_mut::<T>() {
        return Some(ext);
      }
    }
    None
  }
}

#[async_trait]
impl StopperPart for MockContext {
  async fn stop(&self) {}

  async fn poison_pill(&self) {}
}

impl MockContext {
  pub fn new(actor_system: ActorSystem) -> Self {
    Self {
      actor_system,
      extensions: Arc::new(RwLock::new(Vec::new())),
    }
  }
}
