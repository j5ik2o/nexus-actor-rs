//! Mailbox handle implementation.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::dispatch::mailbox::Mailbox;
use crate::actor::message::MessageHandle;

#[derive(Debug)]
pub struct MailboxHandle {
  inner: Arc<RwLock<Box<dyn Mailbox>>>,
}

impl MailboxHandle {
  pub fn new(mailbox: Box<dyn Mailbox>) -> Self {
    Self {
      inner: Arc::new(RwLock::new(mailbox)),
    }
  }
}

#[async_trait]
impl Mailbox for MailboxHandle {
  async fn post_system_message(&self, message: MessageHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    self.inner.write().await.post_system_message(message).await
  }

  async fn post_user_message(&self, message: MessageHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    self.inner.write().await.post_user_message(message).await
  }

  async fn receive_system_message(&self) -> Option<MessageHandle> {
    self.inner.write().await.receive_system_message().await
  }

  async fn receive_user_message(&self) -> Option<MessageHandle> {
    self.inner.write().await.receive_user_message().await
  }
}
