use std::marker::PhantomData;
use std::sync::Arc;

use crate::collections::{PriorityMessage, QueueError, QueueSize, DEFAULT_PRIORITY, PRIORITY_LEVELS};
use crate::collections::{QueueBase, QueueReader, QueueSupport, QueueWriter};
use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub struct PriorityQueue<E, Q> {
  priority_queues: Arc<RwLock<Vec<Q>>>,
  phantom_data: PhantomData<E>,
}

impl<E, Q> PriorityQueue<E, Q>
where
  E: PriorityMessage,
  Q: Clone + QueueReader<E> + QueueWriter<E> + QueueSupport,
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

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    let mut item_priority = DEFAULT_PRIORITY;
    if let Some(priority) = element.get_priority() {
      item_priority = priority.clamp(0, PRIORITY_LEVELS as i8 - 1);
    }
    let mut guard = self.priority_queues.write();
    guard[item_priority as usize].offer(element)
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    let mut guard = self.priority_queues.write();
    for p in (0..PRIORITY_LEVELS).rev() {
      if let Ok(Some(item)) = guard[p].poll() {
        return Ok(Some(item));
      }
    }
    Ok(None)
  }

  pub fn clean_up_shared(&self) {
    let mut guard = self.priority_queues.write();
    for queue in guard.iter_mut() {
      queue.clean_up();
    }
  }

  pub fn len_shared(&self) -> usize {
    let guard = self.priority_queues.read();
    guard.iter().map(|queue| queue.len().to_usize()).sum()
  }
}

impl<E, Q> QueueBase<E> for PriorityQueue<E, Q>
where
  E: PriorityMessage,
  Q: QueueReader<E> + QueueWriter<E> + QueueSupport,
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

impl<E, Q> QueueReader<E> for PriorityQueue<E, Q>
where
  E: PriorityMessage,
  Q: QueueReader<E> + QueueWriter<E> + QueueSupport,
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

impl<E, Q> QueueWriter<E> for PriorityQueue<E, Q>
where
  E: PriorityMessage,
  Q: QueueReader<E> + QueueWriter<E> + QueueSupport,
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

impl<E, Q> QueueSupport for PriorityQueue<E, Q>
where
  E: PriorityMessage,
  Q: QueueReader<E> + QueueWriter<E> + QueueSupport,
{
}

#[cfg(test)]
mod tests;
