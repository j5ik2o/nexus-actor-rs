use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::message::MessageHandle;
use nexus_actor_utils_rs::collections::{Element, Queue, QueueBase, QueueReader, QueueWriter};

impl Element for MessageHandle {
  fn clone_element(&self) -> Self {
    self.clone()
  }
}

#[derive(Debug)]
pub struct UnboundedQueue<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> {
  queue: Arc<RwLock<Q>>,
}

impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> QueueBase<MessageHandle> for UnboundedQueue<Q> {
  fn len(&self) -> usize {
    self.queue.read().await.len()
  }

  fn is_empty(&self) -> bool {
    self.queue.read().await.is_empty()
  }
}

impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> QueueReader<MessageHandle> for UnboundedQueue<Q> {
  async fn dequeue(&mut self) -> Option<MessageHandle> {
    self.queue.write().await.dequeue().await
  }

  async fn peek(&self) -> Option<&MessageHandle> {
    self.queue.read().await.peek().await
  }
}

impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> QueueWriter<MessageHandle> for UnboundedQueue<Q> {
  async fn enqueue(&mut self, item: MessageHandle) {
    self.queue.write().await.enqueue(item).await;
  }
}

impl<Q: QueueReader<MessageHandle> + QueueWriter<MessageHandle>> UnboundedQueue<Q> {
  pub fn new(queue: Q) -> Self {
    Self {
      queue: Arc::new(RwLock::new(queue)),
    }
  }
}
