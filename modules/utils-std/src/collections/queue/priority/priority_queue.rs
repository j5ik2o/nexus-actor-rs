use std::vec::Vec;

use crate::RingQueue;
use nexus_utils_core_rs::{
  PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedQueue, DEFAULT_PRIORITY,
  PRIORITY_LEVELS,
};

#[derive(Debug, Clone)]
pub struct PriorityQueue<E> {
  queues: Vec<RingQueue<E>>,
}

impl<E> PriorityQueue<E>
where
  E: PriorityMessage,
{
  pub fn new(capacity_per_level: usize) -> Self {
    let mut queues = Vec::with_capacity(PRIORITY_LEVELS);
    for _ in 0..PRIORITY_LEVELS {
      queues.push(RingQueue::new(capacity_per_level));
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

impl<E> QueueBase<E> for PriorityQueue<E>
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

impl<E> QueueWriter<E> for PriorityQueue<E>
where
  E: PriorityMessage,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E> QueueReader<E> for PriorityQueue<E>
where
  E: PriorityMessage,
{
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up_mut(&mut self) {
    self.clean_up_shared();
  }
}

impl<E> SharedQueue<E> for PriorityQueue<E>
where
  E: PriorityMessage,
{
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&self) {
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
  fn priority_queue_orders_elements() {
    let queue = PriorityQueue::new(4);
    queue.offer_shared(Msg(10, 1)).unwrap();
    queue.offer_shared(Msg(99, 7)).unwrap();
    queue.offer_shared(Msg(20, 3)).unwrap();

    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 99);
    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 20);
    assert_eq!(queue.poll_shared().unwrap().unwrap().0, 10);
    assert!(queue.poll_shared().unwrap().is_none());
  }

  #[test]
  fn priority_queue_len_capacity_and_clean_up() {
    let queue = PriorityQueue::new(2);
    assert_eq!(queue.len_shared(), QueueSize::limited(0));

    queue.offer_shared(Msg(1, 0)).unwrap();
    assert_eq!(queue.len_shared(), QueueSize::limited(1));

    queue.clean_up_shared();
    assert_eq!(queue.len_shared(), QueueSize::limited(0));
  }

  #[test]
  fn priority_queue_capacity_reflects_levels() {
    let queue = PriorityQueue::<Msg>::new(1);
    assert!(queue.capacity().is_limitless());
  }

  #[test]
  fn priority_queue_offer_via_trait() {
    let mut queue = PriorityQueue::new(2);
    queue.offer_mut(Msg(5, 2)).unwrap();
    assert_eq!(queue.poll_mut().unwrap().unwrap().0, 5);
  }
}
