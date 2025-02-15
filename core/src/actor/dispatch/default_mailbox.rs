//! Default mailbox implementation.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::dispatch::mailbox_message::MailboxMessage;
use crate::actor::message::{Message, MessageHandle};
use nexus_actor_utils_rs::collections::{Element, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

#[derive(Debug)]
pub struct DefaultMailbox {
  system_mailbox: Arc<RwLock<Box<dyn QueueWriter<MessageHandle> + QueueReader<MessageHandle> + Send + Sync>>>,
  user_mailbox: Arc<RwLock<Box<dyn QueueWriter<MessageHandle> + QueueReader<MessageHandle> + Send + Sync>>>,
}

impl DefaultMailbox {
  pub fn new(
    system_mailbox: Box<dyn QueueWriter<MessageHandle> + QueueReader<MessageHandle> + Send + Sync>,
    user_mailbox: Box<dyn QueueWriter<MessageHandle> + QueueReader<MessageHandle> + Send + Sync>,
  ) -> Self {
    Self {
      system_mailbox: Arc::new(RwLock::new(system_mailbox)),
      user_mailbox: Arc::new(RwLock::new(user_mailbox)),
    }
  }

  pub async fn post_system_message(&self, message: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    self.system_mailbox.write().await.offer(message).await
  }

  pub async fn post_user_message(&self, message: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    self.user_mailbox.write().await.offer(message).await
  }

  pub async fn receive_system_message(&self) -> Option<MessageHandle> {
    self.system_mailbox.write().await.poll().await.ok().flatten()
  }

  pub async fn receive_user_message(&self) -> Option<MessageHandle> {
    self.user_mailbox.write().await.poll().await.ok().flatten()
  }
}
