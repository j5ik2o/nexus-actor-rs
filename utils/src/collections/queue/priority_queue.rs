use std::marker::PhantomData;
use std::sync::Arc;

use crate::collections::element::Element;
use crate::collections::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};
use async_trait::async_trait;
use tokio::sync::RwLock;

pub const PRIORITY_LEVELS: usize = 8;
pub const DEFAULT_PRIORITY: i8 = (PRIORITY_LEVELS / 2) as i8;

pub trait PriorityMessage: Element {
  fn get_priority(&self) -> Option<i8>;
}

#[derive(Debug, Clone)]
pub struct PriorityQueue<E, Q> {
  priority_queues: Arc<RwLock<Vec<Q>>>,
  phantom_data: PhantomData<E>,
}

impl<E: PriorityMessage, Q: Clone + QueueReader<E> + QueueWriter<E>> PriorityQueue<E, Q> {
  pub fn new(queue_producer: impl Fn() -> Q + 'static) -> Self {
    let mut queues = Vec::with_capacity(PRIORITY_LEVELS);
    for _ in 0..PRIORITY_LEVELS {
      let queue = queue_producer();
      queues.push(queue);
    }
    Self {
      priority_queues: Arc::new(RwLock::new(queues)),
      phantom_data: PhantomData,
    }
  }
}

#[async_trait]
impl<E: PriorityMessage, Q: QueueReader<E> + QueueWriter<E>> QueueBase<E> for PriorityQueue<E, Q> {
  async fn len(&self) -> QueueSize {
    let queues_mg = self.priority_queues.read().await;
    let mut len = QueueSize::Limited(0);
    for queue in queues_mg.iter() {
      len = len + queue.len().await;
    }
    len
  }

  async fn capacity(&self) -> QueueSize {
    let queues_mg = self.priority_queues.read().await;
    let mut capacity = QueueSize::Limited(0);
    for queue in queues_mg.iter() {
      capacity = capacity + queue.capacity().await;
    }
    capacity
  }
}

#[async_trait]
impl<E: PriorityMessage, Q: QueueReader<E> + QueueWriter<E>> QueueReader<E> for PriorityQueue<E, Q> {
  async fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    for p in (0..PRIORITY_LEVELS).rev() {
      let mut priority_queues_mg = self.priority_queues.write().await;
      if let Ok(Some(item)) = priority_queues_mg[p].poll().await {
        return Ok(Some(item));
      }
    }
    Ok(None)
  }

  async fn clean_up(&mut self) {
    let mut mg = self.priority_queues.write().await;
    for queue in mg.iter_mut() {
      queue.clean_up().await;
    }
  }
}

#[async_trait]
impl<E: PriorityMessage, Q: QueueReader<E> + QueueWriter<E>> QueueWriter<E> for PriorityQueue<E, Q> {
  async fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    let mut item_priority = DEFAULT_PRIORITY;
    if let Some(priority) = element.get_priority() {
      item_priority = priority;
      if item_priority < 0 {
        item_priority = 0;
      }
      if item_priority >= PRIORITY_LEVELS as i8 - 1 {
        item_priority = PRIORITY_LEVELS as i8 - 1;
      }
    }
    let mut mg = self.priority_queues.write().await;
    mg[item_priority as usize].offer(element).await
  }
}
