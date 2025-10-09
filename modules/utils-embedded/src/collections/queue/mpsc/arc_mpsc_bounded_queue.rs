use crate::sync::{ArcShared, ArcStateCell};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use nexus_utils_core_rs::{
  Element, MpscBuffer, MpscQueue, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter,
  RingBufferBackend,
};

/// `Arc`-based bounded MPSC queue with configurable mutex backend
///
/// This queue provides Multi-Producer-Single-Consumer semantics with a fixed capacity,
/// using `Arc` for thread-safe reference counting. The mutex backend is configurable
/// via the `RM` type parameter, allowing selection between `NoopRawMutex` for
/// single-threaded or interrupt-free contexts, and `CriticalSectionRawMutex` for
/// interrupt-safe critical sections.
///
/// # Type Parameters
///
/// * `E` - Element type stored in the queue
/// * `RM` - Raw mutex type (defaults to `NoopRawMutex`)
#[derive(Debug)]
pub struct ArcMpscBoundedQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: MpscQueue<ArcShared<RingBufferBackend<ArcStateCell<MpscBuffer<E>, RM>>>, E>,
}

impl<E, RM> Clone for ArcMpscBoundedQueue<E, RM>
where
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

/// Type alias for `ArcMpscBoundedQueue` using `NoopRawMutex`
///
/// Suitable for single-threaded or interrupt-free contexts where no locking is required.
pub type ArcLocalMpscBoundedQueue<E> = ArcMpscBoundedQueue<E, NoopRawMutex>;

/// Type alias for `ArcMpscBoundedQueue` using `CriticalSectionRawMutex`
///
/// Provides interrupt-safe critical section protection for multi-threaded embedded contexts.
pub type ArcCsMpscBoundedQueue<E> = ArcMpscBoundedQueue<E, CriticalSectionRawMutex>;

impl<E, RM> ArcMpscBoundedQueue<E, RM>
where
  RM: RawMutex,
{
  /// Creates a new bounded MPSC queue with the specified capacity
  ///
  /// # Arguments
  ///
  /// * `capacity` - Maximum number of elements the queue can hold
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_embedded_rs::ArcMpscBoundedQueue;
  ///
  /// let queue: ArcMpscBoundedQueue<i32> = ArcMpscBoundedQueue::new(10);
  /// ```
  pub fn new(capacity: usize) -> Self {
    let storage = ArcShared::new(RingBufferBackend::new(ArcStateCell::new(MpscBuffer::new(Some(
      capacity,
    )))));
    Self {
      inner: MpscQueue::new(storage),
    }
  }
}

impl<E, RM> QueueBase<E> for ArcMpscBoundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E, RM> QueueWriter<E> for ArcMpscBoundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E, RM> QueueReader<E> for ArcMpscBoundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E, RM> QueueRw<E> for ArcMpscBoundedQueue<E, RM>
where
  E: Element,
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

  #[test]
  fn arc_bounded_capacity_limit() {
    prepare();
    let queue: ArcMpscBoundedQueue<u32> = ArcMpscBoundedQueue::new(1);
    queue.offer(10).unwrap();
    let err = queue.offer(11).unwrap_err();
    assert!(matches!(err, QueueError::Full(11)));
  }

  #[test]
  fn arc_bounded_clean_up_closes_queue() {
    prepare();
    let queue: ArcMpscBoundedQueue<u32> = ArcMpscBoundedQueue::new(2);
    queue.offer(1).unwrap();
    queue.clean_up();

    assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer(2), Err(QueueError::Closed(2))));
  }

  #[test]
  fn arc_bounded_reports_len_and_capacity() {
    prepare();
    let queue: ArcMpscBoundedQueue<u32> = ArcMpscBoundedQueue::new(3);
    assert_eq!(queue.capacity(), QueueSize::limited(3));

    queue.offer(1).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));
  }

  #[test]
  fn arc_bounded_trait_cleanup_marks_closed() {
    prepare();
    let queue: ArcMpscBoundedQueue<u32> = ArcMpscBoundedQueue::new(2);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();

    queue.clean_up();
    assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer(3), Err(QueueError::Closed(3))));
  }
}
