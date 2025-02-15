//! Unbounded queue implementation.

use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use nexus_actor_utils_rs::collections::{Element, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

#[derive(Debug, Clone)]
pub struct UnboundedQueue<T: Element> {
  queue: Arc<RwLock<Vec<T>>>,
}

impl<T: Element> UnboundedQueue<T> {
  pub fn new() -> Self {
    Self {
      queue: Arc::new(RwLock::new(Vec::new())),
    }
  }
}

#[async_trait]
impl<T: Element> QueueBase<T> for UnboundedQueue<T> {
  async fn len(&self) -> QueueSize {
    QueueSize::new(self.queue.read().await.len())
  }

  async fn is_empty(&self) -> bool {
    self.queue.read().await.is_empty()
  }
}

#[async_trait]
impl<T: Element> QueueReader<T> for UnboundedQueue<T> {
  async fn poll(&mut self) -> Result<Option<T>, QueueError<T>> {
    Ok(self.queue.write().await.pop())
  }

  async fn clean_up(&mut self) {
    self.queue.write().await.clear();
  }
}

#[async_trait]
impl<T: Element> QueueWriter<T> for UnboundedQueue<T> {
  async fn offer(&mut self, element: T) -> Result<(), QueueError<T>> {
    self.queue.write().await.push(element);
    Ok(())
  }
}

impl<T: Element> Default for UnboundedQueue<T> {
  fn default() -> Self {
    Self::new()
  }
}
