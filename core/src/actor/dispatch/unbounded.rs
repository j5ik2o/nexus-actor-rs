//! Unbounded queue implementation.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::dispatch::mailbox_queue::MailboxQueue;
use crate::actor::message::MessageHandle;
use nexus_actor_utils_rs::collections::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

#[derive(Debug)]
pub struct UnboundedQueue {
  queue: Arc<RwLock<Vec<MessageHandle>>>,
}

impl UnboundedQueue {
  pub fn new() -> Self {
    Self {
      queue: Arc::new(RwLock::new(Vec::new())),
    }
  }
}

#[async_trait]
impl QueueBase<MessageHandle> for UnboundedQueue {
  async fn len(&self) -> QueueSize {
    QueueSize::new(self.queue.read().await.len())
  }

  async fn is_empty(&self) -> bool {
    self.queue.read().await.is_empty()
  }

  async fn capacity(&self) -> QueueSize {
    QueueSize::new(usize::MAX)
  }
}

#[async_trait]
impl QueueReader<MessageHandle> for UnboundedQueue {
  async fn poll(&mut self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    Ok(self.queue.write().await.pop())
  }

  async fn clean_up(&mut self) {
    self.queue.write().await.clear();
  }
}

#[async_trait]
impl QueueWriter<MessageHandle> for UnboundedQueue {
  async fn offer(&mut self, element: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    self.queue.write().await.push(element);
    Ok(())
  }
}

#[async_trait]
impl MailboxQueue for UnboundedQueue {}

impl Default for UnboundedQueue {
  fn default() -> Self {
    Self::new()
  }
}
