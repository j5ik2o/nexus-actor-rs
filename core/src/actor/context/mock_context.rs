use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::context::actor_context::{
  ActorSystem, Context, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart, RootContext,
  SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart, TypedContext,
};
use crate::actor::{ActorError, Message, MessageHandle, MessageOrEnvelope, Pid, Props, SpawnError};

#[derive(Debug)]
pub struct MockContext {
  inner: Arc<RwLock<ActorSystem>>,
}

#[async_trait]
impl Context for MockContext {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[async_trait]
impl InfoPart for MockContext {
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
impl MessagePart for MockContext {
  async fn get_message_headers_opt(&self) -> Option<Arc<RwLock<dyn Any + Send + Sync>>> {
    None
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
impl SenderPart for MockContext {
  async fn send(&self, _target: &Pid, _message: MessageHandle) {}

  async fn request(&self, _target: &Pid, _message: MessageHandle) -> Result<Box<dyn Message>, ActorError> {
    unimplemented!()
  }

  async fn forward(&self, _target: &Pid, _message: MessageHandle) {}
}

#[async_trait]
impl ExtensionPart for MockContext {
  async fn get_extension<T: Any + Send + Sync>(&self) -> Option<Arc<RwLock<T>>> {
    None
  }

  async fn set_extension<T: Any + Send + Sync>(&self, _extension: T) {}
}

#[async_trait]
impl StopperPart for MockContext {
  async fn stop(&self, _pid: &Pid) {}

  async fn poison_pill(&self, _pid: &Pid) {}

  async fn watch(&self, _pid: &Pid) {}

  async fn unwatch(&self, _pid: &Pid) {}

  async fn handle_failure(&self, _who: Option<Pid>, _error: ActorError, _message: Option<MessageHandle>) {}
}
