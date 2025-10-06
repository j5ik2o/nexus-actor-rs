use core::cell::RefCell;

use nexus_utils_core_rs::{
  Element, MpscBuffer, MpscQueue, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, RingBufferBackend,
  SharedQueue,
};

use crate::sync::RcShared;

#[derive(Debug, Clone)]
pub struct RcMpscBoundedQueue<E> {
  inner: MpscQueue<RcShared<RingBufferBackend<RefCell<MpscBuffer<E>>>>, E>,
}

impl<E> RcMpscBoundedQueue<E> {
  pub fn new(capacity: usize) -> Self {
    let storage = RcShared::new(RingBufferBackend::new(RefCell::new(MpscBuffer::new(Some(capacity)))));
    Self {
      inner: MpscQueue::new(storage),
    }
  }
}

impl<E: Element> QueueBase<E> for RcMpscBoundedQueue<E> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E: Element> QueueWriter<E> for RcMpscBoundedQueue<E> {
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E: Element> QueueReader<E> for RcMpscBoundedQueue<E> {
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E: Element> SharedQueue<E> for RcMpscBoundedQueue<E> {
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
  use nexus_utils_core_rs::{QueueBase, SharedQueue};

  #[test]
  fn rc_bounded_capacity_limit() {
    let queue: RcMpscBoundedQueue<u32> = RcMpscBoundedQueue::new(1);
    queue.offer(42).unwrap();
    let err = queue.offer(99).unwrap_err();
    assert!(matches!(err, QueueError::Full(99)));
  }

  #[test]
  fn rc_bounded_clean_up_closes_queue() {
    let queue: RcMpscBoundedQueue<u32> = RcMpscBoundedQueue::new(2);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();

    queue.clean_up();
    assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer(3), Err(QueueError::Closed(3))));
  }

  #[test]
  fn rc_bounded_capacity_tracking() {
    let queue: RcMpscBoundedQueue<u32> = RcMpscBoundedQueue::new(2);
    assert_eq!(queue.capacity().to_usize(), 2);
    queue.offer(1).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));
  }
}
