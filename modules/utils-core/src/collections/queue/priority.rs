use crate::collections::{
  element::Element,
  queue::{QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter},
};
use alloc::vec::Vec;
use core::marker::PhantomData;

/// Number of priority queue levels
///
/// By default, supports 8 priority levels.
/// Ranges from 0 (lowest priority) to 7 (highest priority).
pub const PRIORITY_LEVELS: usize = 8;

/// Default priority level
///
/// Used when message priority is not specified.
/// Defaults to the midpoint of PRIORITY_LEVELS (4).
pub const DEFAULT_PRIORITY: i8 = (PRIORITY_LEVELS / 2) as i8;

/// Trait for messages with priority
///
/// Implementing this trait allows messages to have priority.
/// Priority is specified in the range 0 to 7, with higher values indicating higher priority.
pub trait PriorityMessage: Element {
  /// Gets the message priority
  ///
  /// # Returns
  ///
  /// * `Some(i8)` - Priority in range 0 to 7; higher value = higher priority
  /// * `None` - If priority is not specified, default priority is used
  fn get_priority(&self) -> Option<i8>;
}

/// Priority queue
///
/// A facade for queues with multiple priority levels.
/// Distributes messages to the appropriate level queue based on priority.
/// When removing, processes from the highest priority queue first.
///
/// # Type Parameters
///
/// * `Q` - Queue type used for each level. Must implement `QueueRw<E>` trait
/// * `E` - Type of elements stored in queue. Must implement `PriorityMessage` trait
#[derive(Debug)]
pub struct PriorityQueue<Q, E>
where
  Q: QueueRw<E>, {
  levels: Vec<Q>,
  _marker: PhantomData<E>,
}

impl<Q, E> PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
{
  /// Creates a new priority queue
  ///
  /// # Arguments
  ///
  /// * `levels` - Vector of queues corresponding to each priority level.
  ///              Index 0 is lowest priority, last index is highest priority
  ///
  /// # Panics
  ///
  /// Panics if `levels` is empty
  pub fn new(levels: Vec<Q>) -> Self {
    assert!(!levels.is_empty(), "PriorityQueue requires at least one level");
    Self {
      levels,
      _marker: PhantomData,
    }
  }

  /// Gets immutable references to queues at each level
  ///
  /// # Returns
  ///
  /// Slice of queues for each priority level
  pub fn levels(&self) -> &[Q] {
    &self.levels
  }

  /// Gets mutable references to queues at each level
  ///
  /// # Returns
  ///
  /// Mutable slice of queues for each priority level
  pub fn levels_mut(&mut self) -> &mut [Q] {
    &mut self.levels
  }

  /// Calculates level index from priority
  ///
  /// If priority is out of range, it is clamped to the range 0 to max (number of levels - 1).
  ///
  /// # Arguments
  ///
  /// * `priority` - Message priority. If None, uses default value (middle level)
  ///
  /// # Returns
  ///
  /// Index in range 0 to levels.len()-1
  fn level_index(&self, priority: Option<i8>) -> usize {
    let levels = self.levels.len();
    let default = (levels / 2) as i8;
    let max = (levels as i32 - 1) as i8;
    let clamped = priority.unwrap_or(default).clamp(0, max);
    clamped as usize
  }

  /// Adds an element to the queue
  ///
  /// Based on the element's priority, adds it to the appropriate level queue.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If successfully added
  /// * `Err(QueueError)` - If could not add due to reasons such as queue being full
  pub fn offer(&self, element: E) -> Result<(), QueueError<E>>
  where
    E: PriorityMessage, {
    let idx = self.level_index(element.get_priority());
    self.levels[idx].offer(element)
  }

  /// Removes an element from the queue
  ///
  /// Removes elements from the highest priority queue first.
  /// Returns `None` if all queues are empty.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - If element was removed
  /// * `Ok(None)` - If all queues are empty
  /// * `Err(QueueError)` - If an error occurred
  pub fn poll(&self) -> Result<Option<E>, QueueError<E>>
  where
    E: PriorityMessage, {
    for queue in self.levels.iter().rev() {
      match queue.poll()? {
        Some(item) => return Ok(Some(item)),
        None => continue,
      }
    }
    Ok(None)
  }

  /// Cleans up queues at all levels
  ///
  /// Organizes internal state of each queue and releases unnecessary resources.
  pub fn clean_up(&self) {
    for queue in &self.levels {
      queue.clean_up();
    }
  }

  /// Calculates total length of queues at all levels
  ///
  /// # Returns
  ///
  /// Sum of number of elements stored in all queues.
  /// Returns `QueueSize::Limitless` if any queue is unlimited.
  fn aggregate_len(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.levels {
      match queue.len() {
        QueueSize::Limitless => return QueueSize::limitless(),
        QueueSize::Limited(value) => total += value,
      }
    }
    QueueSize::limited(total)
  }

  /// Calculates total capacity of queues at all levels
  ///
  /// # Returns
  ///
  /// Sum of capacities of all queues.
  /// Returns `QueueSize::Limitless` if any queue is unlimited.
  fn aggregate_capacity(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.levels {
      match queue.capacity() {
        QueueSize::Limitless => return QueueSize::limitless(),
        QueueSize::Limited(value) => total += value,
      }
    }
    QueueSize::limited(total)
  }
}

impl<Q, E> Clone for PriorityQueue<Q, E>
where
  Q: QueueRw<E> + Clone,
{
  fn clone(&self) -> Self {
    Self {
      levels: self.levels.clone(),
      _marker: PhantomData,
    }
  }
}

impl<Q, E> QueueBase<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  /// Returns total length of queues at all levels
  ///
  /// # Returns
  ///
  /// Sum of number of elements stored in all queues
  fn len(&self) -> QueueSize {
    self.aggregate_len()
  }

  /// Returns total capacity of queues at all levels
  ///
  /// # Returns
  ///
  /// Sum of capacities of all queues
  fn capacity(&self) -> QueueSize {
    self.aggregate_capacity()
  }
}

impl<Q, E> QueueWriter<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  /// Adds an element to the queue (mutable reference version)
  ///
  /// Based on the element's priority, adds it to the appropriate level queue.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If successfully added
  /// * `Err(QueueError)` - If could not add due to reasons such as queue being full
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }
}

impl<Q, E> QueueReader<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  /// Removes an element from the queue (mutable reference version)
  ///
  /// Removes elements from the highest priority queue first.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - If element was removed
  /// * `Ok(None)` - If all queues are empty
  /// * `Err(QueueError)` - If an error occurred
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  /// Cleans up queues at all levels (mutable reference version)
  ///
  /// Organizes internal state of each queue and releases unnecessary resources.
  fn clean_up_mut(&mut self) {
    self.clean_up();
  }
}

impl<Q, E> QueueRw<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  /// Adds an element to the queue
  ///
  /// Based on the element's priority, adds it to the appropriate level queue.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If successfully added
  /// * `Err(QueueError)` - If could not add due to reasons such as queue being full
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }

  /// Removes an element from the queue
  ///
  /// Removes elements from the highest priority queue first.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - If element was removed
  /// * `Ok(None)` - If all queues are empty
  /// * `Err(QueueError)` - If an error occurred
  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  /// Cleans up queues at all levels
  ///
  /// Organizes internal state of each queue and releases unnecessary resources.
  fn clean_up(&self) {
    self.clean_up();
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;
  use crate::collections::queue::priority::Vec;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  use super::{PriorityMessage, PriorityQueue};
  use crate::collections::queue::mpsc::{MpscBuffer, MpscHandle, MpscQueue, RingBufferBackend};
  use crate::collections::queue::{QueueBase, QueueReader, QueueRw, QueueWriter};
  use crate::collections::{QueueError, QueueSize};
  use crate::sync::Shared;

  #[derive(Debug, Clone)]
  struct TestQueue(MpscQueue<RcHandle<u32>, u32>);

  #[derive(Debug)]
  struct RcHandle<T>(Rc<RingBufferBackend<RefCell<MpscBuffer<T>>>>);

  impl<T> RcHandle<T> {
    fn new(capacity: Option<usize>) -> Self {
      let buffer = RefCell::new(MpscBuffer::new(capacity));
      let backend = RingBufferBackend::new(buffer);
      Self(Rc::new(backend))
    }
  }

  impl<T> core::ops::Deref for RcHandle<T> {
    type Target = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> Clone for RcHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> Shared<RingBufferBackend<RefCell<MpscBuffer<T>>>> for RcHandle<T> {}

  impl<T> MpscHandle<T> for RcHandle<T> {
    type Backend = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn backend(&self) -> &Self::Backend {
      &self.0
    }
  }

  impl PriorityMessage for u32 {
    fn get_priority(&self) -> Option<i8> {
      Some((*self % 8) as i8)
    }
  }

  impl QueueRw<u32> for TestQueue {
    fn offer(&self, element: u32) -> Result<(), QueueError<u32>> {
      self.0.offer(element)
    }

    fn poll(&self) -> Result<Option<u32>, QueueError<u32>> {
      self.0.poll()
    }

    fn clean_up(&self) {
      self.0.clean_up();
    }
  }

  impl QueueBase<u32> for TestQueue {
    fn len(&self) -> QueueSize {
      self.0.len()
    }

    fn capacity(&self) -> QueueSize {
      self.0.capacity()
    }
  }

  impl QueueWriter<u32> for TestQueue {
    fn offer_mut(&mut self, element: u32) -> Result<(), QueueError<u32>> {
      self.0.offer_mut(element)
    }
  }

  impl QueueReader<u32> for TestQueue {
    fn poll_mut(&mut self) -> Result<Option<u32>, QueueError<u32>> {
      self.0.poll_mut()
    }

    fn clean_up_mut(&mut self) {
      self.0.clean_up_mut();
    }
  }

  impl TestQueue {
    fn bounded(cap: usize) -> Self {
      Self(MpscQueue::new(RcHandle::new(Some(cap))))
    }

    fn unbounded() -> Self {
      Self(MpscQueue::new(RcHandle::new(None)))
    }
  }

  fn sample_levels() -> Vec<TestQueue> {
    (0..super::PRIORITY_LEVELS).map(|_| TestQueue::bounded(4)).collect()
  }

  #[test]
  fn shared_priority_queue_orders_by_priority() {
    let queue = PriorityQueue::new(sample_levels());
    queue.offer(1).unwrap();
    queue.offer(15).unwrap();
    queue.offer(7).unwrap();

    assert_eq!(queue.poll().unwrap(), Some(15));
    assert_eq!(queue.poll().unwrap(), Some(7));
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), None);
  }

  #[test]
  fn shared_priority_queue_len_and_capacity() {
    let queue = PriorityQueue::new(sample_levels());
    let expected = QueueSize::limited(super::PRIORITY_LEVELS * 4);
    assert_eq!(queue.capacity(), expected);
    queue.offer(3).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));
    queue.clean_up();
    assert_eq!(queue.len(), QueueSize::limited(0));
  }

  #[test]
  fn shared_priority_queue_unbounded_capacity() {
    let levels = (0..super::PRIORITY_LEVELS).map(|_| TestQueue::unbounded()).collect();
    let queue = PriorityQueue::new(levels);
    assert!(queue.capacity().is_limitless());
  }
}
