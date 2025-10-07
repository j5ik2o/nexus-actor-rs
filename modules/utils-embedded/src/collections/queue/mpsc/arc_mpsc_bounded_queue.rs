use crate::sync::{ArcShared, ArcStateCell};
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use nexus_utils_core_rs::{
  Element, MpscBuffer, MpscQueue, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter,
  RingBufferBackend,
};

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

pub type ArcLocalMpscBoundedQueue<E> = ArcMpscBoundedQueue<E, NoopRawMutex>;
pub type ArcCsMpscBoundedQueue<E> = ArcMpscBoundedQueue<E, CriticalSectionRawMutex>;

impl<E, RM> ArcMpscBoundedQueue<E, RM>
where
  RM: RawMutex,
{
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
