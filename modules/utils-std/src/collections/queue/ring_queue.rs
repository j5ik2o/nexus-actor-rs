use std::sync::Mutex;

use crate::sync::ArcShared;
use nexus_utils_core_rs::{
  QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, RingBuffer, SharedQueue, SharedRingQueue,
  DEFAULT_CAPACITY,
};

#[derive(Debug, Clone)]
pub struct RingQueue<E> {
  inner: SharedRingQueue<ArcShared<Mutex<RingBuffer<E>>>, E>,
}

impl<E> RingQueue<E> {
  pub fn new(capacity: usize) -> Self {
    let storage = ArcShared::new(Mutex::new(RingBuffer::new(capacity)));
    Self {
      inner: SharedRingQueue::new(storage),
    }
  }

  pub fn with_dynamic(mut self, dynamic: bool) -> Self {
    self.inner = self.inner.with_dynamic(dynamic);
    self
  }

  pub fn set_dynamic(&self, dynamic: bool) {
    self.inner.set_dynamic(dynamic);
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_shared(element)
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_shared()
  }

  pub fn clean_up_shared(&self) {
    self.inner.clean_up_shared();
  }

  pub fn len_shared(&self) -> QueueSize {
    self.inner.len_shared()
  }

  pub fn capacity_shared(&self) -> QueueSize {
    self.inner.capacity_shared()
  }
}

impl<E> Default for RingQueue<E> {
  fn default() -> Self {
    Self::new(DEFAULT_CAPACITY)
  }
}

impl<E> QueueBase<E> for RingQueue<E> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E> QueueWriter<E> for RingQueue<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }
}

impl<E> QueueReader<E> for RingQueue<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  fn clean_up(&mut self) {
    self.inner.clean_up();
  }
}

impl<E> SharedQueue<E> for RingQueue<E> {
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_shared(element)
  }

  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_shared()
  }

  fn clean_up_shared(&self) {
    self.inner.clean_up_shared();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ring_queue_offer_poll() {
    let queue = RingQueue::new(2).with_dynamic(false);
    queue.offer_shared(1).unwrap();
    queue.offer_shared(2).unwrap();
    assert_eq!(queue.offer_shared(3), Err(QueueError::Full(3)));

    assert_eq!(queue.poll_shared().unwrap(), Some(1));
    assert_eq!(queue.poll_shared().unwrap(), Some(2));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }

  #[test]
  fn ring_queue_shared_clone_observes_state() {
    let queue = RingQueue::new(4);
    let cloned = queue.clone();

    queue.offer_shared(10).unwrap();
    queue.offer_shared(11).unwrap();

    assert_eq!(cloned.len_shared().to_usize(), 2);
    assert_eq!(cloned.poll_shared().unwrap(), Some(10));
    assert_eq!(queue.poll_shared().unwrap(), Some(11));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }
}
