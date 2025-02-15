//! Unbounded queue implementation.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::message::MessageHandle;
use nexus_actor_utils_rs::collections::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

#[derive(Debug, Clone)]
pub struct UnboundedQueue<T: Debug + Send + Sync + 'static> {
  queue: Arc<RwLock<Vec<T>>>,
}

impl<T: Debug + Send + Sync + 'static> UnboundedQueue<T> {
  pub fn new() -> Self {
    Self {
      queue: Arc::new(RwLock::new(Vec::new())),
    }
  }
}

#[async_trait]
impl<T: Debug + Send + Sync + 'static> QueueBase for UnboundedQueue<T> {
  async fn len(&self) -> QueueSize {
    QueueSize::new(self.queue.read().await.len())
  }

  async fn is_empty(&self) -> bool {
    self.queue.read().await.is_empty()
  }
}

#[async_trait]
impl<T: Debug + Send + Sync + 'static> QueueReader<T> for UnboundedQueue<T> {
  async fn dequeue(&mut self) -> Option<T> {
    self.queue.write().await.pop()
  }

  async fn peek(&self) -> Option<&T> {
    self.queue.read().await.last()
  }
}

#[async_trait]
impl<T: Debug + Send + Sync + 'static> QueueWriter<T> for UnboundedQueue<T> {
  async fn enqueue(&mut self, item: T) -> Result<(), QueueError> {
    self.queue.write().await.push(item);
    Ok(())
  }
}

impl<T: Debug + Send + Sync + 'static> Default for UnboundedQueue<T> {
  fn default() -> Self {
    Self::new()
  }
}
