//! Default mailbox implementation.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::dispatch::mailbox::{Mailbox, MailboxQueue};
use crate::actor::message::MessageHandle;

#[derive(Debug)]
pub struct DefaultMailbox {
  system_mailbox: Arc<RwLock<MailboxQueue>>,
  user_mailbox: Arc<RwLock<MailboxQueue>>,
}

impl DefaultMailbox {
  pub fn new(system_mailbox: MailboxQueue, user_mailbox: MailboxQueue) -> Self {
    Self {
      system_mailbox: Arc::new(RwLock::new(system_mailbox)),
      user_mailbox: Arc::new(RwLock::new(user_mailbox)),
    }
  }
}

#[async_trait]
impl Mailbox for DefaultMailbox {
  async fn post_system_message(&self, message: MessageHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    self
      .system_mailbox
      .write()
      .await
      .offer(message)
      .await
      .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
  }

  async fn post_user_message(&self, message: MessageHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    self
      .user_mailbox
      .write()
      .await
      .offer(message)
      .await
      .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
  }

  async fn receive_system_message(&self) -> Option<MessageHandle> {
    self.system_mailbox.write().await.poll().await.ok().flatten()
  }

  async fn receive_user_message(&self) -> Option<MessageHandle> {
    self.user_mailbox.write().await.poll().await.ok().flatten()
  }
}
