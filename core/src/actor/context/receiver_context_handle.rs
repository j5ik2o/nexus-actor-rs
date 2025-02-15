use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::context::actor_context::{Context, InfoPart, MessagePart, ReceiverContext, ReceiverPart};
use crate::actor::{ActorSystem, Message, MessageHandle, MessageOrEnvelope, Pid};

#[derive(Debug)]
pub struct ReceiverContextHandle {
  inner: Arc<RwLock<ActorSystem>>,
}

#[async_trait]
impl Context for ReceiverContextHandle {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[async_trait]
impl InfoPart for ReceiverContextHandle {
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
impl MessagePart for ReceiverContextHandle {
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
impl ReceiverPart for ReceiverContextHandle {
  async fn receive(&self, _message: MessageHandle) {}
}

impl ReceiverContext for ReceiverContextHandle {}
