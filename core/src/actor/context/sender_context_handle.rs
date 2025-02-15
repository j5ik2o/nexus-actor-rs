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
pub struct SenderContextHandle(pub Arc<RwLock<dyn ActorContext>>);

impl Context for SenderContextHandle {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[async_trait]
impl InfoPart for SenderContextHandle {
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
impl MessagePart for SenderContextHandle {
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
impl SenderPart for SenderContextHandle {
  async fn forward(&self, pid: &Pid) {
    self.0.read().await.forward(pid).await
  }

  async fn respond(&self, response: ResponseHandle) {
    self.0.read().await.respond(response).await
  }

  async fn get_sender(&self) -> Option<Pid> {
    self.0.read().await.get_sender().await
  }

  async fn send(&mut self, pid: Pid, message_handle: MessageHandle) {
    self.0.write().await.send(pid, message_handle).await
  }

  async fn request(&mut self, pid: Pid, message_handle: MessageHandle) {
    self.0.write().await.request(pid, message_handle).await
  }

  async fn request_with_custom_sender(&mut self, pid: Pid, message_handle: MessageHandle, sender: Pid) {
    self
      .0
      .write()
      .await
      .request_with_custom_sender(pid, message_handle, sender)
      .await
  }

  async fn request_future(&self, pid: Pid, message_handle: MessageHandle) -> Result<ResponseHandle, ActorError> {
    self.0.read().await.request_future(pid, message_handle).await
  }
}

impl SenderContext for SenderContextHandle {}
