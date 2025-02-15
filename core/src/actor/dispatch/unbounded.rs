use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::message::MessageHandle;
use nexus_actor_utils_rs::collections::{Element, Queue, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

impl Element for MessageHandle {
  fn clone_element(&self) -> Self {
    self.clone()
  }
}

#[derive(Debug)]
pub struct UnboundedQueue<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> {
  queue: Arc<RwLock<Q>>,
}

#[async_trait]
impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> QueueBase<MessageHandle> for UnboundedQueue<Q> {
  async fn len(&self) -> usize {
    self.queue.read().await.len().await
  }

  async fn is_empty(&self) -> bool {
    self.queue.read().await.is_empty().await
  }

  async fn capacity(&self) -> QueueSize {
    self.queue.read().await.capacity().await
  }
}

#[async_trait]
impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> QueueReader<MessageHandle> for UnboundedQueue<Q> {
  async fn dequeue(&mut self) -> Option<MessageHandle> {
    self.queue.write().await.dequeue().await
  }

  async fn peek(&self) -> Option<&MessageHandle> {
    self.queue.read().await.peek().await
  }

  async fn poll(&mut self) -> Result<Option<MessageHandle>, QueueError<MessageHandle>> {
    self.queue.write().await.poll().await
  }

  async fn clean_up(&mut self) {
    self.queue.write().await.clean_up().await;
  }
}

#[async_trait]
impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> QueueWriter<MessageHandle> for UnboundedQueue<Q> {
  async fn enqueue(&mut self, item: MessageHandle) {
    self.queue.write().await.enqueue(item).await;
  }

  async fn offer(&mut self, item: MessageHandle) -> Result<(), QueueError<MessageHandle>> {
    self.queue.write().await.offer(item).await
  }
}

impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> UnboundedQueue<Q> {
  pub fn new(queue: Q) -> Self {
    Self {
      queue: Arc::new(RwLock::new(queue)),
    }
  }
}
