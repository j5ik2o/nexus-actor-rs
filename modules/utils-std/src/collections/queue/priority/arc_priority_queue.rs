use crate::ArcRingQueue;
use nexus_utils_core_rs::{
  PriorityMessage, PriorityQueue, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter, PRIORITY_LEVELS,
};

/// Priority queue
///
/// A priority queue that retrieves elements based on message priority.
/// Internally uses multiple ring queue levels to process higher priority messages first.
#[derive(Debug, Clone)]
pub struct ArcPriorityQueue<E> {
  inner: PriorityQueue<ArcRingQueue<E>, E>,
}

impl<E> ArcPriorityQueue<E> {
  /// Creates a new priority queue with the specified capacity per priority level
  ///
  /// # Arguments
  ///
  /// * `capacity_per_level` - Queue capacity for each priority level
  ///
  /// # Returns
  ///
  /// A new priority queue instance
  pub fn new(capacity_per_level: usize) -> Self {
    let levels = (0..PRIORITY_LEVELS)
      .map(|_| ArcRingQueue::new(capacity_per_level))
      .collect();
    Self {
      inner: PriorityQueue::new(levels),
    }
  }

  /// Returns an immutable reference to the priority level array
  ///
  /// # Returns
  ///
  /// A slice of priority level queues
  pub fn levels(&self) -> &[ArcRingQueue<E>] {
    self.inner.levels()
  }

  /// Returns a mutable reference to the priority level array
  ///
  /// # Returns
  ///
  /// A mutable slice of priority level queues
  pub fn levels_mut(&mut self) -> &mut [ArcRingQueue<E>] {
    self.inner.levels_mut()
  }

  /// Returns an immutable reference to the internal priority queue
  ///
  /// # Returns
  ///
  /// A reference to the internal priority queue
  pub fn inner(&self) -> &PriorityQueue<ArcRingQueue<E>, E> {
    &self.inner
  }

  /// Returns a mutable reference to the internal priority queue
  ///
  /// # Returns
  ///
  /// A mutable reference to the internal priority queue
  pub fn inner_mut(&mut self) -> &mut PriorityQueue<ArcRingQueue<E>, E> {
    &mut self.inner
  }
}

impl<E> ArcPriorityQueue<E>
where
  E: PriorityMessage,
{
  /// Adds an element to the queue based on its priority
  ///
  /// # Arguments
  ///
  /// * `element` - The element to add to the queue
  ///
  /// # Returns
  ///
  /// `Ok(())` on success, `Err(QueueError::Full)` if the queue is full
  pub fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }

  /// Retrieves the highest priority element from the queue
  ///
  /// # Returns
  ///
  /// `Ok(Some(E))` on success, `Ok(None)` if the queue is empty
  pub fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  /// Clears all elements in the queue
  pub fn clean_up(&self) {
    self.inner.clean_up();
  }
}

impl<E> QueueBase<E> for ArcPriorityQueue<E>
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

impl<E> QueueWriter<E> for ArcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E> QueueReader<E> for ArcPriorityQueue<E>
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

impl<E> QueueRw<E> for ArcPriorityQueue<E>
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
    let queue = ArcPriorityQueue::new(4);
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
    let queue = ArcPriorityQueue::new(2);
    assert_eq!(queue.len(), QueueSize::limited(0));

    queue.offer(Msg(1, 0)).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));

    queue.clean_up();
    assert_eq!(queue.len(), QueueSize::limited(0));
  }

  #[test]
  fn priority_queue_capacity_reflects_levels() {
    let queue = ArcPriorityQueue::<Msg>::new(1);
    assert!(queue.capacity().is_limitless());
  }

  #[test]
  fn priority_queue_offer_via_trait() {
    let mut queue = ArcPriorityQueue::new(2);
    queue.offer_mut(Msg(5, 2)).unwrap();
    assert_eq!(queue.poll_mut().unwrap().unwrap().0, 5);
  }
}
