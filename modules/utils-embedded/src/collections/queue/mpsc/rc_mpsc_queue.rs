use core::cell::RefCell;

use nexus_utils_core_rs::{
  Element, MpscBuffer, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedMpscQueue, SharedQueue,
};

use crate::sync::RcShared;

#[derive(Debug, Clone)]
pub struct RcMpscUnboundedQueue<E> {
  inner: SharedMpscQueue<RcShared<RefCell<MpscBuffer<E>>>, E>,
}

impl<E> RcMpscUnboundedQueue<E> {
  pub fn new() -> Self {
    let storage = RcShared::new(RefCell::new(MpscBuffer::new(None)));
    Self {
      inner: SharedMpscQueue::new(storage),
    }
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>>
  where
    E: Element, {
    self.inner.offer_shared(element)
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>>
  where
    E: Element, {
    self.inner.poll_shared()
  }

  pub fn clean_up_shared(&self) {
    self.inner.clean_up_shared();
  }

  pub fn len_shared(&self) -> QueueSize {
    self.inner.len_shared()
  }
}

impl<E> Default for RcMpscUnboundedQueue<E> {
  fn default() -> Self {
    Self::new()
  }
}

impl<E: Element> QueueBase<E> for RcMpscUnboundedQueue<E> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E: Element> QueueWriter<E> for RcMpscUnboundedQueue<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }
}

impl<E: Element> QueueReader<E> for RcMpscUnboundedQueue<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  fn clean_up(&mut self) {
    self.inner.clean_up();
  }
}

impl<E: Element> SharedQueue<E> for RcMpscUnboundedQueue<E> {
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }

  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up_shared(&self) {
    self.clean_up_shared();
  }
}

#[derive(Debug, Clone)]
pub struct RcMpscBoundedQueue<E> {
  inner: SharedMpscQueue<RcShared<RefCell<MpscBuffer<E>>>, E>,
}

impl<E> RcMpscBoundedQueue<E> {
  pub fn new(capacity: usize) -> Self {
    let storage = RcShared::new(RefCell::new(MpscBuffer::new(Some(capacity))));
    Self {
      inner: SharedMpscQueue::new(storage),
    }
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>>
  where
    E: Element, {
    self.inner.offer_shared(element)
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>>
  where
    E: Element, {
    self.inner.poll_shared()
  }

  pub fn clean_up_shared(&self) {
    self.inner.clean_up_shared();
  }

  pub fn len_shared(&self) -> QueueSize {
    self.inner.len_shared()
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
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }
}

impl<E: Element> QueueReader<E> for RcMpscBoundedQueue<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  fn clean_up(&mut self) {
    self.inner.clean_up();
  }
}

impl<E: Element> SharedQueue<E> for RcMpscBoundedQueue<E> {
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }

  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up_shared(&self) {
    self.clean_up_shared();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rc_unbounded_offer_poll() {
    let queue: RcMpscUnboundedQueue<u32> = RcMpscUnboundedQueue::new();
    queue.offer_shared(1).unwrap();
    queue.offer_shared(2).unwrap();
    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll_shared().unwrap(), Some(1));
    assert_eq!(queue.poll_shared().unwrap(), Some(2));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }

  #[test]
  fn rc_unbounded_clean_up_signals_disconnected() {
    let queue: RcMpscUnboundedQueue<u8> = RcMpscUnboundedQueue::new();
    queue.offer_shared(1).unwrap();
    queue.clean_up_shared();

    assert!(matches!(queue.poll_shared(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer_shared(2), Err(QueueError::Closed(2))));
  }

  #[test]
  fn rc_bounded_capacity_limit() {
    let queue: RcMpscBoundedQueue<u32> = RcMpscBoundedQueue::new(1);
    queue.offer_shared(42).unwrap();
    let err = queue.offer_shared(99).unwrap_err();
    assert!(matches!(err, QueueError::Full(99)));
  }

  #[test]
  fn rc_bounded_clean_up_closes_queue() {
    let queue: RcMpscBoundedQueue<u32> = RcMpscBoundedQueue::new(2);
    queue.offer_shared(1).unwrap();
    queue.offer_shared(2).unwrap();

    queue.clean_up_shared();
    assert!(matches!(queue.poll_shared(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer_shared(3), Err(QueueError::Closed(3))));
  }

  #[test]
  fn rc_bounded_capacity_tracking() {
    let queue: RcMpscBoundedQueue<u32> = RcMpscBoundedQueue::new(2);
    assert_eq!(queue.capacity().to_usize(), 2);
    queue.offer_shared(1).unwrap();
    assert_eq!(queue.len_shared(), QueueSize::limited(1));
  }

  #[test]
  fn rc_unbounded_offer_poll_via_traits() {
    let mut queue: RcMpscUnboundedQueue<u32> = RcMpscUnboundedQueue::new();
    queue.offer(1).unwrap();
    assert_eq!(queue.poll().unwrap(), Some(1));
  }
}
