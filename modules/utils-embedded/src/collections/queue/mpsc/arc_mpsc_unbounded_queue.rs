use crate::sync::{ArcShared, ArcStateCell};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use nexus_utils_core_rs::{
  Element, MpscBuffer, MpscQueue, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter,
  RingBufferBackend,
};

/// `Arc`-based unbounded MPSC queue with configurable mutex backend
///
/// This queue provides Multi-Producer-Single-Consumer semantics with dynamic capacity,
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
pub struct ArcMpscUnboundedQueue<E, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: MpscQueue<ArcShared<RingBufferBackend<ArcStateCell<MpscBuffer<E>, RM>>>, E>,
}

impl<E, RM> Clone for ArcMpscUnboundedQueue<E, RM>
where
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

/// Type alias for `ArcMpscUnboundedQueue` using `NoopRawMutex`
///
/// Suitable for single-threaded or interrupt-free contexts where no locking is required.
pub type ArcLocalMpscUnboundedQueue<E> = ArcMpscUnboundedQueue<E, NoopRawMutex>;

/// Type alias for `ArcMpscUnboundedQueue` using `CriticalSectionRawMutex`
///
/// Provides interrupt-safe critical section protection for multi-threaded embedded contexts.
pub type ArcCsMpscUnboundedQueue<E> = ArcMpscUnboundedQueue<E, CriticalSectionRawMutex>;

impl<E, RM> ArcMpscUnboundedQueue<E, RM>
where
  RM: RawMutex,
{
  /// Creates a new unbounded MPSC queue
  ///
  /// The queue will dynamically grow as needed to accommodate elements.
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_embedded_rs::ArcMpscUnboundedQueue;
  ///
  /// let queue: ArcMpscUnboundedQueue<i32> = ArcMpscUnboundedQueue::new();
  /// ```
  pub fn new() -> Self {
    let storage = ArcShared::new(RingBufferBackend::new(ArcStateCell::new(MpscBuffer::new(None))));
    Self {
      inner: MpscQueue::new(storage),
    }
  }
}

impl<E, RM> QueueBase<E> for ArcMpscUnboundedQueue<E, RM>
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

impl<E, RM> QueueWriter<E> for ArcMpscUnboundedQueue<E, RM>
where
  E: Element,
  RM: RawMutex,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E, RM> QueueReader<E> for ArcMpscUnboundedQueue<E, RM>
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

impl<E, RM> QueueRw<E> for ArcMpscUnboundedQueue<E, RM>
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
  fn arc_unbounded_offer_poll() {
    prepare();
    let queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), Some(2));
    assert_eq!(queue.poll().unwrap(), None);
  }

  #[test]
  fn arc_unbounded_clean_up_signals_disconnect() {
    prepare();
    let queue: ArcMpscUnboundedQueue<u8> = ArcMpscUnboundedQueue::new();
    queue.offer(9).unwrap();
    queue.clean_up();

    assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer(1), Err(QueueError::Closed(1))));
  }

  #[test]
  fn arc_unbounded_offer_poll_via_traits() {
    prepare();
    let queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    queue.offer(7).unwrap();
    assert_eq!(queue.poll().unwrap(), Some(7));
  }

  #[test]
  fn arc_unbounded_capacity_reports_limitless() {
    prepare();
    let queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    assert!(queue.capacity().is_limitless());
  }
}
