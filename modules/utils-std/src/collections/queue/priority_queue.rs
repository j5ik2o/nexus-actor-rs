use std::marker::PhantomData;
use std::sync::Arc;

use crate::collections::queue_sync::{SyncQueueBase, SyncQueueReader, SyncQueueSupport, SyncQueueWriter};
use crate::collections::{PriorityMessage, QueueError, QueueSize, DEFAULT_PRIORITY, PRIORITY_LEVELS};
use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub struct PriorityQueue<E, Q> {
  priority_queues: Arc<RwLock<Vec<Q>>>,
  phantom_data: PhantomData<E>,
}

impl<E, Q> PriorityQueue<E, Q>
where
  E: PriorityMessage,
  Q: Clone + SyncQueueReader<E> + SyncQueueWriter<E> + SyncQueueSupport,
{
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

impl<E, Q> SyncQueueBase<E> for PriorityQueue<E, Q>
where
  E: PriorityMessage,
  Q: SyncQueueReader<E> + SyncQueueWriter<E> + SyncQueueSupport,
{
  fn len(&self) -> QueueSize {
    let queues_guard = self.priority_queues.read();
    let mut len = QueueSize::Limited(0);
    for queue in queues_guard.iter() {
      len = len + queue.len();
    }
    len
  }

  fn capacity(&self) -> QueueSize {
    let queues_guard = self.priority_queues.read();
    let mut capacity = QueueSize::Limited(0);
    for queue in queues_guard.iter() {
      capacity = capacity + queue.capacity();
    }
    capacity
  }
}

impl<E, Q> SyncQueueReader<E> for PriorityQueue<E, Q>
where
  E: PriorityMessage,
  Q: SyncQueueReader<E> + SyncQueueWriter<E> + SyncQueueSupport,
{
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    let mut guard = self.priority_queues.write();
    for p in (0..PRIORITY_LEVELS).rev() {
      if let Ok(Some(item)) = guard[p].poll() {
        return Ok(Some(item));
      }
    }
    Ok(None)
  }

  fn clean_up(&mut self) {
    let mut guard = self.priority_queues.write();
    for queue in guard.iter_mut() {
      queue.clean_up();
    }
  }
}

impl<E, Q> SyncQueueWriter<E> for PriorityQueue<E, Q>
where
  E: PriorityMessage,
  Q: SyncQueueReader<E> + SyncQueueWriter<E> + SyncQueueSupport,
{
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
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
    let mut guard = self.priority_queues.write();
    guard[item_priority as usize].offer(element)
  }
}

impl<E, Q> SyncQueueSupport for PriorityQueue<E, Q>
where
  E: PriorityMessage,
  Q: SyncQueueReader<E> + SyncQueueWriter<E> + SyncQueueSupport,
{
}

#[cfg(test)]
mod tests;
