use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::actor::{
  ActorContext, ActorError, ActorHandle, ActorSystem, Message, MessageHandle, MessageOrEnvelope, Pid, Props,
  ReadonlyMessageHeadersHandle, ResponseHandle,
};

#[derive(Debug)]
pub struct ReceiverContextHandle(pub Arc<RwLock<dyn ActorContext>>);

impl Context for ReceiverContextHandle {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[async_trait]
impl InfoPart for ReceiverContextHandle {
  async fn get_children(&self) -> Vec<Pid> {
    self.0.read().await.get_children().await
  }

  async fn get_receive_timeout(&self) -> Duration {
    self.0.read().await.get_receive_timeout().await
  }

  async fn get_parent(&self) -> Option<Pid> {
    self.0.read().await.get_parent().await
  }

  async fn get_self_opt(&self) -> Option<Pid> {
    self.0.read().await.get_self_opt().await
  }

  async fn set_self(&mut self, pid: Pid) {
    self.0.write().await.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    self.0.read().await.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.0.read().await.get_actor_system().await
  }
}

#[async_trait]
impl MessagePart for ReceiverContextHandle {
  async fn get_message(&self) -> MessageHandle {
    self.0.read().await.get_message().await
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.0.read().await.get_message_header_handle().await
  }

  async fn get_message_envelope_opt(&self) -> Option<MessageOrEnvelope> {
    self.0.read().await.get_message_envelope_opt().await
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    self.0.read().await.get_message_handle_opt().await
  }
}

#[async_trait]
impl ReceiverPart for ReceiverContextHandle {
  async fn receive(&self, envelope: MessageOrEnvelope) -> Result<(), ActorError> {
    self.0.read().await.receive(envelope).await
  }
}

impl ReceiverContext for ReceiverContextHandle {}
