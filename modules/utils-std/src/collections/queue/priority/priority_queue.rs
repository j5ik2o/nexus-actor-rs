use crate::RingQueue;
use nexus_utils_core_rs::{
  PriorityMessage, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedPriorityQueue, SharedQueue,
  PRIORITY_LEVELS,
};

#[derive(Debug, Clone)]
pub struct PriorityQueue<E> {
  inner: SharedPriorityQueue<RingQueue<E>, E>,
}

impl<E> PriorityQueue<E> {
  pub fn new(capacity_per_level: usize) -> Self {
    let levels = (0..PRIORITY_LEVELS)
      .map(|_| RingQueue::new(capacity_per_level))
      .collect();
    Self {
      inner: SharedPriorityQueue::new(levels),
    }
  }

  pub fn levels(&self) -> &[RingQueue<E>] {
    self.inner.levels()
  }

  pub fn levels_mut(&mut self) -> &mut [RingQueue<E>] {
    self.inner.levels_mut()
  }

  pub fn inner(&self) -> &SharedPriorityQueue<RingQueue<E>, E> {
    &self.inner
  }

  pub fn inner_mut(&mut self) -> &mut SharedPriorityQueue<RingQueue<E>, E> {
    &mut self.inner
  }
}

impl<E> PriorityQueue<E>
where
  E: PriorityMessage,
{
  pub fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }

  pub fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  pub fn clean_up(&self) {
    self.inner.clean_up();
  }
}

impl<E> QueueBase<E> for PriorityQueue<E>
where
  E: PriorityMessage,
{
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E> QueueWriter<E> for PriorityQueue<E>
where
  E: PriorityMessage,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E> QueueReader<E> for PriorityQueue<E>
where
  E: PriorityMessage,
{
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E> SharedQueue<E> for PriorityQueue<E>
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
    queue.offer(Msg(10, 1)).unwrap();
    queue.offer(Msg(99, 7)).unwrap();
    queue.offer(Msg(20, 3)).unwrap();

    assert_eq!(queue.poll().unwrap().unwrap().0, 99);
    assert_eq!(queue.poll().unwrap().unwrap().0, 20);
    assert_eq!(queue.poll().unwrap().unwrap().0, 10);
    assert!(queue.poll().unwrap().is_none());
  }

  #[test]
  fn priority_queue_len_capacity_and_clean_up() {
    let queue = PriorityQueue::new(2);
    assert_eq!(queue.len(), QueueSize::limited(0));

    queue.offer(Msg(1, 0)).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));

    queue.clean_up();
    assert_eq!(queue.len(), QueueSize::limited(0));
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
