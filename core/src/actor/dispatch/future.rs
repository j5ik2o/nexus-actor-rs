//! Future process implementation.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::message::Message;
use crate::actor::pid::Pid;
use crate::actor::process::Process;

#[derive(Debug, Clone)]
pub struct ActorFutureProcess {
  inner: Arc<RwLock<Box<dyn Process + Send + Sync>>>,
}

#[async_trait]
impl Process for ActorFutureProcess {
  async fn send_user_message(&self, sender: Option<&Pid>, message: Box<dyn Message>) {
    self.inner.write().await.send_user_message(sender, message).await;
  }

  async fn send_system_message(&self, message: Box<dyn Message>) {
    self.inner.write().await.send_system_message(message).await;
  }

  async fn stop(&self) {
    self.inner.write().await.stop().await;
  }

  async fn set_dead(&self) {
    self.inner.write().await.set_dead().await;
  }
}
