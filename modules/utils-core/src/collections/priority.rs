use super::{
  element::Element,
  queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedQueue},
};
use alloc::vec::Vec;
use core::marker::PhantomData;

pub const PRIORITY_LEVELS: usize = 8;
pub const DEFAULT_PRIORITY: i8 = (PRIORITY_LEVELS / 2) as i8;

pub trait PriorityMessage: Element {
  fn get_priority(&self) -> Option<i8>;
}

/// Shared priority queue facade that distributes messages across per-level queues.
#[derive(Debug)]
pub struct SharedPriorityQueue<Q, E>
where
  Q: SharedQueue<E>, {
  levels: Vec<Q>,
  _marker: PhantomData<E>,
}

impl<Q, E> SharedPriorityQueue<Q, E>
where
  Q: SharedQueue<E>,
{
  pub fn new(levels: Vec<Q>) -> Self {
    assert!(!levels.is_empty(), "SharedPriorityQueue requires at least one level");
    Self {
      levels,
      _marker: PhantomData,
    }
  }

  pub fn levels(&self) -> &[Q] {
    &self.levels
  }

  pub fn levels_mut(&mut self) -> &mut [Q] {
    &mut self.levels
  }

  fn level_index(&self, priority: Option<i8>) -> usize {
    let levels = self.levels.len();
    let default = (levels / 2) as i8;
    let max = (levels as i32 - 1) as i8;
    let clamped = priority.unwrap_or(default).clamp(0, max);
    clamped as usize
  }

  pub fn offer(&self, element: E) -> Result<(), QueueError<E>>
  where
    E: PriorityMessage, {
    let idx = self.level_index(element.get_priority());
    self.levels[idx].offer(element)
  }

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

  pub fn clean_up(&self) {
    for queue in &self.levels {
      queue.clean_up();
    }
  }

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

impl<Q, E> Clone for SharedPriorityQueue<Q, E>
where
  Q: SharedQueue<E> + Clone,
{
  fn clone(&self) -> Self {
    Self {
      levels: self.levels.clone(),
      _marker: PhantomData,
    }
  }
}

impl<Q, E> QueueBase<E> for SharedPriorityQueue<Q, E>
where
  Q: SharedQueue<E>,
  E: PriorityMessage,
{
  fn len(&self) -> QueueSize {
    self.aggregate_len()
  }

  fn capacity(&self) -> QueueSize {
    self.aggregate_capacity()
  }
}

impl<Q, E> QueueWriter<E> for SharedPriorityQueue<Q, E>
where
  Q: SharedQueue<E>,
  E: PriorityMessage,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }
}

impl<Q, E> QueueReader<E> for SharedPriorityQueue<Q, E>
where
  Q: SharedQueue<E>,
  E: PriorityMessage,
{
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  fn clean_up_mut(&mut self) {
    self.clean_up();
  }
}

impl<Q, E> SharedQueue<E> for SharedPriorityQueue<Q, E>
where
  Q: SharedQueue<E>,
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
  extern crate alloc;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  use super::{PriorityMessage, SharedPriorityQueue};
  use crate::collections::queue::mpsc::{MpscBuffer, RingBufferBackend, SharedMpscHandle, SharedMpscQueue};
  use crate::collections::queue::{QueueBase, QueueReader, QueueWriter, SharedQueue};
  use crate::collections::{QueueError, QueueSize};
  use crate::sync::Shared;

  #[derive(Debug, Clone)]
  struct TestQueue(SharedMpscQueue<RcHandle<u32>, u32>);

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

  impl<T> SharedMpscHandle<T> for RcHandle<T> {
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

  impl SharedQueue<u32> for TestQueue {
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
      Self(SharedMpscQueue::new(RcHandle::new(Some(cap))))
    }

    fn unbounded() -> Self {
      Self(SharedMpscQueue::new(RcHandle::new(None)))
    }
  }

  fn sample_levels() -> Vec<TestQueue> {
    (0..super::PRIORITY_LEVELS).map(|_| TestQueue::bounded(4)).collect()
  }

  #[test]
  fn shared_priority_queue_orders_by_priority() {
    let queue = SharedPriorityQueue::new(sample_levels());
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
    let queue = SharedPriorityQueue::new(sample_levels());
    let expected = QueueSize::limited((super::PRIORITY_LEVELS * 4) as usize);
    assert_eq!(queue.capacity(), expected);
    queue.offer(3).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));
    queue.clean_up();
    assert_eq!(queue.len(), QueueSize::limited(0));
  }

  #[test]
  fn shared_priority_queue_unbounded_capacity() {
    let levels = (0..super::PRIORITY_LEVELS).map(|_| TestQueue::unbounded()).collect();
    let queue = SharedPriorityQueue::new(levels);
    assert!(queue.capacity().is_limitless());
  }
}
