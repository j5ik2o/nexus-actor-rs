use alloc::vec::Vec;

use nexus_utils_core_rs::{
  PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, DEFAULT_PRIORITY, PRIORITY_LEVELS,
};

use crate::RcRingQueue;

#[derive(Debug, Clone)]
pub struct RcPriorityQueue<E> {
  queues: Vec<RcRingQueue<E>>,
}

impl<E> RcPriorityQueue<E>
where
  E: PriorityMessage,
{
  pub fn new(capacity_per_level: usize) -> Self {
    let mut queues = Vec::with_capacity(PRIORITY_LEVELS);
    for _ in 0..PRIORITY_LEVELS {
      queues.push(RcRingQueue::new(capacity_per_level));
    }
    Self { queues }
  }

  fn priority_index(&self, element: &E) -> usize {
    element
      .get_priority()
      .unwrap_or(DEFAULT_PRIORITY)
      .clamp(0, PRIORITY_LEVELS as i8 - 1) as usize
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    let idx = self.priority_index(&element);
    self.queues[idx].offer_shared(element)
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    for queue in self.queues.iter().rev() {
      if let Some(item) = queue.poll_shared()? {
        return Ok(Some(item));
      }
    }
    Ok(None)
  }

  pub fn len_shared(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.queues {
      match queue.len_shared() {
        QueueSize::Limitless => return QueueSize::limitless(),
        QueueSize::Limited(value) => total += value,
      }
    }
    QueueSize::limited(total)
  }

  pub fn clean_up_shared(&self) {
    for queue in &self.queues {
      queue.clean_up_shared();
    }
  }
}

impl<E> QueueBase<E> for RcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.queues {
      match queue.capacity_shared() {
        QueueSize::Limitless => return QueueSize::limitless(),
        QueueSize::Limited(value) => total += value,
      }
    }
    QueueSize::limited(total)
  }
}

impl<E> QueueWriter<E> for RcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E> QueueReader<E> for RcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&mut self) {
    self.clean_up_shared();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug, Clone)]
  struct Msg(i32, i8);

  impl nexus_utils_core_rs::Element for Msg {}

  impl PriorityMessage for Msg {
    fn get_priority(&self) -> Option<i8> {
      Some(self.1)
    }
  }

  #[test]
  fn rc_priority_queue_orders() {
    let queue = RcPriorityQueue::new(4);
    queue.offer_shared(Msg(1, 0)).unwrap();
    queue.offer_shared(Msg(5, 7)).unwrap();
    queue.offer_shared(Msg(3, 3)).unwrap();

    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 5);
    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 3);
    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 1);
    assert!(queue.poll_shared().unwrap().is_none());
  }
}
