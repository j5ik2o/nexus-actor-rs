use nexus_utils_core_rs::{
  PriorityMessage, PriorityQueue, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter, PRIORITY_LEVELS,
};

use crate::RcRingQueue;

/// `Rc`-based priority queue
///
/// This queue is available in `no_std` environments and controls processing order
/// based on message priority. It provides reference-counted shared ownership
/// using `Rc` and `RefCell`.
///
/// # Features
///
/// - **Priority-based**: Determines processing order based on message priority (0-7)
/// - **Multiple levels**: Supports 8 priority levels
/// - **Dynamic/static modes**: Choose between dynamic expansion or fixed capacity
/// - **no_std support**: Does not require the standard library
/// - **Cloneable**: Multiple handles can be created via `clone()`
///
/// # Priority
///
/// - Priority ranges from 0 (lowest) to 7 (highest) across 8 levels
/// - Default priority (0) is used when priority is not specified
/// - Higher priority messages are processed first
///
/// # Performance characteristics
///
/// - `offer`: O(1)
/// - `poll`: O(PRIORITY_LEVELS), typically close to O(1)
/// - Memory usage: O(capacity_per_level * PRIORITY_LEVELS)
///
/// # Examples
///
/// ```
/// use nexus_utils_embedded_rs::RcPriorityQueue;
/// use nexus_utils_core_rs::{QueueRw, PriorityMessage};
///
/// #[derive(Debug)]
/// struct Task {
///     id: u32,
///     priority: i8,
/// }
///
/// impl nexus_utils_core_rs::Element for Task {}
///
/// impl PriorityMessage for Task {
///     fn get_priority(&self) -> Option<i8> {
///         Some(self.priority)
///     }
/// }
///
/// let queue = RcPriorityQueue::new(10);
/// queue.offer(Task { id: 1, priority: 0 }).unwrap();
/// queue.offer(Task { id: 2, priority: 5 }).unwrap();
///
/// // Higher priority task is retrieved first
/// let task = queue.poll().unwrap().unwrap();
/// assert_eq!(task.id, 2);
/// ```
#[derive(Debug, Clone)]
pub struct RcPriorityQueue<E> {
  inner: PriorityQueue<RcRingQueue<E>, E>,
}

impl<E> RcPriorityQueue<E> {
  /// Creates a new priority queue with the specified capacity per priority level
  ///
  /// # Arguments
  ///
  /// * `capacity_per_level` - Maximum number of elements that can be stored in each priority level queue
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcPriorityQueue;
  ///
  /// // Can store up to 10 elements per priority level
  /// let queue: RcPriorityQueue<u32> = RcPriorityQueue::new(10);
  /// ```
  pub fn new(capacity_per_level: usize) -> Self {
    let levels = (0..PRIORITY_LEVELS)
      .map(|_| RcRingQueue::new(capacity_per_level))
      .collect();
    Self {
      inner: PriorityQueue::new(levels),
    }
  }

  /// Sets the dynamic expansion mode of the queue
  ///
  /// # Arguments
  ///
  /// * `dynamic` - If `true`, automatically expands when capacity is insufficient.
  ///               If `false`, capacity limits are strictly enforced.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcPriorityQueue;
  ///
  /// let queue: RcPriorityQueue<i32> = RcPriorityQueue::new(5);
  /// queue.set_dynamic(false); // Fixed capacity mode
  /// ```
  pub fn set_dynamic(&self, dynamic: bool) {
    for queue in self.inner.levels() {
      queue.set_dynamic(dynamic);
    }
  }

  /// Sets the dynamic expansion mode and returns self (builder pattern)
  ///
  /// # Arguments
  ///
  /// * `dynamic` - If `true`, automatically expands when capacity is insufficient
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcPriorityQueue;
  ///
  /// let queue: RcPriorityQueue<i32> = RcPriorityQueue::new(5)
  ///     .with_dynamic(false);
  /// ```
  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  /// Returns an immutable reference to the internal priority level queues
  ///
  /// # Returns
  ///
  /// Reference to an array of 8 priority level queues
  pub fn levels(&self) -> &[RcRingQueue<E>] {
    self.inner.levels()
  }

  /// Returns a mutable reference to the internal priority level queues
  ///
  /// # Returns
  ///
  /// Mutable reference to an array of 8 priority level queues
  pub fn levels_mut(&mut self) -> &mut [RcRingQueue<E>] {
    self.inner.levels_mut()
  }

  /// Returns an immutable reference to the internal `PriorityQueue`
  ///
  /// # Returns
  ///
  /// Reference to the internal `PriorityQueue` instance
  pub fn inner(&self) -> &PriorityQueue<RcRingQueue<E>, E> {
    &self.inner
  }
}

impl<E> QueueBase<E> for RcPriorityQueue<E>
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

impl<E> QueueWriter<E> for RcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E> QueueReader<E> for RcPriorityQueue<E>
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

impl<E> QueueRw<E> for RcPriorityQueue<E>
where
  E: PriorityMessage,
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
