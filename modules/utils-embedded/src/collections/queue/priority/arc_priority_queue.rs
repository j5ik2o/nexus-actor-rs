use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use nexus_utils_core_rs::{
  PriorityMessage, PriorityQueue, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter, PRIORITY_LEVELS,
};

use crate::collections::queue::ring::ArcRingQueue;

/// Type alias for `ArcPriorityQueue` using `NoopRawMutex`
///
/// Suitable for single-threaded or interrupt-free contexts where no locking is required.
pub type ArcLocalPriorityQueue<E> = ArcPriorityQueue<E, NoopRawMutex>;

/// Type alias for `ArcPriorityQueue` using `CriticalSectionRawMutex`
///
/// Provides interrupt-safe critical section protection for multi-threaded embedded contexts.
pub type ArcCsPriorityQueue<E> = ArcPriorityQueue<E, CriticalSectionRawMutex>;

/// `Arc`-based priority queue with configurable mutex backend
///
/// This queue provides priority-based message ordering with 8 priority levels (0-7),
/// using `Arc` for thread-safe reference counting. The mutex backend is configurable
/// via the `RM` type parameter.
///
/// # Type Parameters
///
/// * `E` - Element type, must implement `PriorityMessage`
/// * `RM` - Raw mutex type (defaults to `NoopRawMutex`)
#[derive(Debug)]
pub struct ArcPriorityQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: PriorityQueue<ArcRingQueue<E, RM>, E>,
}

impl<E, RM> Clone for ArcPriorityQueue<E, RM>
where
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

impl<E, RM> ArcPriorityQueue<E, RM>
where
  RM: RawMutex,
{
  /// Creates a new priority queue with the specified capacity per priority level
  ///
  /// # Arguments
  ///
  /// * `capacity_per_level` - Maximum number of elements per priority level
  pub fn new(capacity_per_level: usize) -> Self {
    let levels = (0..PRIORITY_LEVELS)
      .map(|_| ArcRingQueue::new(capacity_per_level))
      .collect();
    Self {
      inner: PriorityQueue::new(levels),
    }
  }

  /// Sets the dynamic expansion mode for all priority levels
  ///
  /// # Arguments
  ///
  /// * `dynamic` - If `true`, queues expand dynamically; if `false`, capacity is fixed
  pub fn set_dynamic(&self, dynamic: bool) {
    for queue in self.inner.levels() {
      queue.set_dynamic(dynamic);
    }
  }

  /// Sets the dynamic expansion mode and returns self (builder pattern)
  ///
  /// # Arguments
  ///
  /// * `dynamic` - If `true`, queues expand dynamically
  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  /// Returns immutable references to the internal priority level queues
  ///
  /// # Returns
  ///
  /// Slice of 8 priority level queues
  pub fn levels(&self) -> &[ArcRingQueue<E, RM>] {
    self.inner.levels()
  }

  /// Returns mutable references to the internal priority level queues
  ///
  /// # Returns
  ///
  /// Mutable slice of 8 priority level queues
  pub fn levels_mut(&mut self) -> &mut [ArcRingQueue<E, RM>] {
    self.inner.levels_mut()
  }

  /// Returns a reference to the internal `PriorityQueue`
  ///
  /// # Returns
  ///
  /// Reference to the underlying priority queue implementation
  pub fn inner(&self) -> &PriorityQueue<ArcRingQueue<E, RM>, E> {
    &self.inner
  }
}

impl<E, RM> QueueBase<E> for ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E, RM> QueueWriter<E> for ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
  RM: RawMutex,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E, RM> QueueReader<E> for ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
  RM: RawMutex,
{
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E, RM> QueueRw<E> for ArcPriorityQueue<E, RM>
where
  E: PriorityMessage,
  RM: RawMutex,
{
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  fn clean_up(&self) {
    self.inner.clean_up();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tests::init_arc_critical_section;
  use nexus_utils_core_rs::{QueueBase, QueueRw};

  fn prepare() {
    init_arc_critical_section();
  }

  #[derive(Debug, Clone)]
  struct Msg(i32, i8);

  impl nexus_utils_core_rs::Element for Msg {}

  impl PriorityMessage for Msg {
    fn get_priority(&self) -> Option<i8> {
      Some(self.1)
    }
  }

  #[test]
  fn arc_priority_queue_orders() {
    prepare();
    let queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(3);
    queue.offer(Msg(1, 0)).unwrap();
    queue.offer(Msg(9, 7)).unwrap();
    queue.offer(Msg(5, 3)).unwrap();

    assert_eq!(queue.poll().unwrap().unwrap().0, 9);
    assert_eq!(queue.poll().unwrap().unwrap().0, 5);
    assert_eq!(queue.poll().unwrap().unwrap().0, 1);
    assert!(queue.poll().unwrap().is_none());
  }

  #[test]
  fn arc_priority_queue_len_and_clean_up() {
    prepare();
    let queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(1);
    queue.offer(Msg(1, 0)).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));
    queue.clean_up();
    assert_eq!(queue.len(), QueueSize::limited(0));
  }

  #[test]
  fn arc_priority_queue_len_across_levels() {
    prepare();
    let queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(2);
    queue.offer(Msg(1, 0)).unwrap();
    queue.offer(Msg(2, 5)).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(2));
  }

  #[test]
  fn arc_priority_queue_capacity_behaviour() {
    prepare();
    let queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(2);
    assert!(queue.capacity().is_limitless());

    queue.set_dynamic(false);
    let expected = QueueSize::limited(2 * PRIORITY_LEVELS);
    assert_eq!(queue.capacity(), expected);
  }

  #[test]
  fn arc_priority_queue_trait_cleanup() {
    prepare();
    let queue: ArcLocalPriorityQueue<Msg> = ArcLocalPriorityQueue::new(2).with_dynamic(false);
    queue.offer(Msg(1, 0)).unwrap();
    queue.offer(Msg(2, 1)).unwrap();
    assert_eq!(queue.poll().unwrap().unwrap().0, 2);
    queue.clean_up();
    assert!(queue.poll().unwrap().is_none());
  }

  #[test]
  fn arc_priority_queue_priority_clamp_and_default() {
    prepare();
    #[derive(Debug, Clone)]
    struct OptionalPriority(i32, Option<i8>);

    impl nexus_utils_core_rs::Element for OptionalPriority {}

    impl PriorityMessage for OptionalPriority {
      fn get_priority(&self) -> Option<i8> {
        self.1
      }
    }

    let queue: ArcLocalPriorityQueue<OptionalPriority> = ArcLocalPriorityQueue::new(1).with_dynamic(false);
    queue.offer(OptionalPriority(1, Some(127))).unwrap();
    queue.offer(OptionalPriority(2, Some(-128))).unwrap();
    queue.offer(OptionalPriority(3, None)).unwrap();

    assert_eq!(queue.poll().unwrap().unwrap().0, 1);
    assert_eq!(queue.poll().unwrap().unwrap().0, 3);
    assert_eq!(queue.poll().unwrap().unwrap().0, 2);
  }
}
