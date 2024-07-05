use std::sync::Arc;
use std::time::Duration;

use crate::actor::actor::actor_handle::ActorHandle;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{InfoPart, MessagePart, SenderContext, SenderPart};
use crate::actor::future::Future;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::readonly_message_headers::ReadonlyMessageHeadersHandle;
use async_trait::async_trait;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct SenderContextHandle(Arc<Mutex<dyn SenderContext>>);

impl SenderContextHandle {
  pub fn new_arc(context: Arc<Mutex<dyn SenderContext>>) -> Self {
    SenderContextHandle(context)
  }

  pub fn new(c: impl SenderContext + 'static) -> Self {
    SenderContextHandle(Arc::new(Mutex::new(c)))
  }
}

#[async_trait]
impl InfoPart for SenderContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_parent().await
  }

  async fn get_self(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_self().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let mg = self.0.lock().await;
    mg.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let mg = self.0.lock().await;
    mg.get_actor_system().await
  }
}

#[async_trait]
impl SenderPart for SenderContextHandle {
  async fn get_sender(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_sender().await
  }

  async fn send(&mut self, pid: ExtendedPid, message: MessageHandle) {
    let mut mg = self.0.lock().await;
    mg.send(pid, message).await
  }

  async fn request(&mut self, pid: ExtendedPid, message: MessageHandle) {
    let mut mg = self.0.lock().await;
    mg.request(pid, message).await
  }

  async fn request_with_custom_sender(&mut self, pid: ExtendedPid, message: MessageHandle, sender: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.request_with_custom_sender(pid, message, sender).await
  }

  async fn request_future(&self, pid: ExtendedPid, message: MessageHandle, timeout: &Duration) -> Future {
    let mg = self.0.lock().await;
    mg.request_future(pid, message, timeout).await
  }
}

#[async_trait]
impl MessagePart for SenderContextHandle {
  async fn get_message(&self) -> Option<MessageHandle> {
    let mg = self.0.lock().await;
    mg.get_message().await
  }

  async fn get_message_header(&self) -> Option<ReadonlyMessageHeadersHandle> {
    let mg = self.0.lock().await;
    mg.get_message_header().await
  }
}

impl SenderContext for SenderContextHandle {}
