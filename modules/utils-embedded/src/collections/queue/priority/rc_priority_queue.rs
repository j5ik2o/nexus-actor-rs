use alloc::vec::Vec;

use nexus_utils_core_rs::{
  PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedQueue, DEFAULT_PRIORITY,
  PRIORITY_LEVELS,
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

  pub fn set_dynamic(&self, dynamic: bool) {
    for queue in &self.queues {
      queue.set_dynamic(dynamic);
    }
  }

  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  fn priority_index(&self, element: &E) -> usize {
    element
      .get_priority()
      .unwrap_or(DEFAULT_PRIORITY)
      .clamp(0, PRIORITY_LEVELS as i8 - 1) as usize
  }

  pub fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    let idx = self.priority_index(&element);
    self.queues[idx].offer(element)
  }

  pub fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    for queue in self.queues.iter().rev() {
      if let Some(item) = queue.poll()? {
        return Ok(Some(item));
      }
    }
    Ok(None)
  }

  pub fn clean_up(&self) {
    for queue in &self.queues {
      queue.clean_up();
    }
  }

  fn total_len(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.queues {
      match queue.len() {
        QueueSize::Limitless => return QueueSize::limitless(),
        QueueSize::Limited(value) => total += value,
      }
    }
    QueueSize::limited(total)
  }
}

impl<E> QueueBase<E> for RcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn len(&self) -> QueueSize {
    self.total_len()
  }

  fn capacity(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.queues {
      match queue.capacity() {
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
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }
}

impl<E> QueueReader<E> for RcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  fn clean_up_mut(&mut self) {
    self.clean_up();
  }
}

impl<E> SharedQueue<E> for RcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  fn clean_up(&self) {
    self.clean_up();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use nexus_utils_core_rs::{QueueBase, QueueReader, QueueWriter};

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
    let queue: RcPriorityQueue<Msg> = RcPriorityQueue::new(4);
    queue.offer(Msg(1, 0)).unwrap();
    queue.offer(Msg(5, 7)).unwrap();
    queue.offer(Msg(3, 3)).unwrap();

    assert_eq!(queue.poll().unwrap().unwrap().0, 5);
    assert_eq!(queue.poll().unwrap().unwrap().0, 3);
    assert_eq!(queue.poll().unwrap().unwrap().0, 1);
    assert!(queue.poll().unwrap().is_none());
  }

  #[test]
  fn rc_priority_queue_len_capacity_updates() {
    let queue: RcPriorityQueue<Msg> = RcPriorityQueue::new(2);
    assert_eq!(queue.len(), QueueSize::limited(0));

    queue.offer(Msg(1, 0)).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));

    queue.clean_up();
    assert_eq!(queue.len(), QueueSize::limited(0));
  }

  #[test]
  fn rc_priority_queue_len_across_levels() {
    let queue: RcPriorityQueue<Msg> = RcPriorityQueue::new(2);
    queue.offer(Msg(1, 0)).unwrap();
    queue.offer(Msg(2, 5)).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(2));
  }

  #[test]
  fn rc_priority_queue_capacity_behaviour() {
    let queue: RcPriorityQueue<Msg> = RcPriorityQueue::new(1);
    assert!(queue.capacity().is_limitless());

    queue.set_dynamic(false);
    let expected = QueueSize::limited(PRIORITY_LEVELS);
    assert_eq!(queue.capacity(), expected);
  }

  #[test]
  fn rc_priority_queue_trait_cleanup() {
    let mut queue: RcPriorityQueue<Msg> = RcPriorityQueue::new(2).with_dynamic(false);
    queue.offer_mut(Msg(1, 0)).unwrap();
    queue.offer_mut(Msg(2, 1)).unwrap();
    assert_eq!(queue.poll_mut().unwrap().unwrap().0, 2);
    queue.clean_up_mut();
    assert!(queue.poll_mut().unwrap().is_none());
  }

  #[test]
  fn rc_priority_queue_priority_clamp_and_default() {
    #[derive(Debug, Clone)]
    struct OptionalPriority(i32, Option<i8>);

    impl nexus_utils_core_rs::Element for OptionalPriority {}

    impl PriorityMessage for OptionalPriority {
      fn get_priority(&self) -> Option<i8> {
        self.1
      }
    }

    let queue: RcPriorityQueue<OptionalPriority> = RcPriorityQueue::new(1).with_dynamic(false);
    queue.offer(OptionalPriority(1, Some(127))).unwrap();
    queue.offer(OptionalPriority(2, Some(-128))).unwrap();
    queue.offer(OptionalPriority(3, None)).unwrap();

    assert_eq!(queue.poll().unwrap().unwrap().0, 1);
    assert_eq!(queue.poll().unwrap().unwrap().0, 3);
    assert_eq!(queue.poll().unwrap().unwrap().0, 2);
  }
}
