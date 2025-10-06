use std::sync::Mutex;

use crate::sync::ArcShared;
use nexus_utils_core_rs::{
  QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter, RingBuffer, RingQueue, DEFAULT_CAPACITY,
};

#[derive(Debug, Clone)]
pub struct ArcRingQueue<E> {
  inner: RingQueue<ArcShared<Mutex<RingBuffer<E>>>, E>,
}

impl<E> ArcRingQueue<E> {
  pub fn new(capacity: usize) -> Self {
    let storage = ArcShared::new(Mutex::new(RingBuffer::new(capacity)));
    Self {
      inner: RingQueue::new(storage),
    }
  }

  pub fn with_dynamic(mut self, dynamic: bool) -> Self {
    self.inner = self.inner.with_dynamic(dynamic);
    self
  }

  pub fn set_dynamic(&self, dynamic: bool) {
    self.inner.set_dynamic(dynamic);
  }
}

impl<E> Default for ArcRingQueue<E> {
  fn default() -> Self {
    Self::new(DEFAULT_CAPACITY)
  }
}

impl<E> QueueBase<E> for ArcRingQueue<E> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E> QueueWriter<E> for ArcRingQueue<E> {
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E> QueueReader<E> for ArcRingQueue<E> {
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E> QueueRw<E> for ArcRingQueue<E> {
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

  #[test]
  fn ring_queue_offer_poll() {
    let queue = ArcRingQueue::new(2).with_dynamic(false);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert_eq!(queue.offer(3), Err(QueueError::Full(3)));

    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), Some(2));
    assert_eq!(queue.poll().unwrap(), None);
  }

  #[test]
  fn ring_queue_shared_clone_observes_state() {
    let queue = ArcRingQueue::new(4);
    let cloned = queue.clone();

    queue.offer(10).unwrap();
    queue.offer(11).unwrap();

    assert_eq!(cloned.len().to_usize(), 2);
    assert_eq!(cloned.poll().unwrap(), Some(10));
    assert_eq!(queue.poll().unwrap(), Some(11));
    assert_eq!(queue.poll().unwrap(), None);
  }

  #[test]
  fn ring_queue_clean_up_resets_queue() {
    let queue = ArcRingQueue::new(2);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();

    queue.clean_up();
    assert_eq!(queue.len().to_usize(), 0);
    assert!(queue.poll().unwrap().is_none());
  }

  #[test]
  fn ring_queue_dynamic_resize() {
    let queue = ArcRingQueue::new(1);
    queue.set_dynamic(true);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert!(queue.len().to_usize() >= 2);
  }

  #[test]
  fn ring_queue_capacity_and_poll_via_traits() {
    let mut queue = ArcRingQueue::new(1).with_dynamic(false);
    queue.offer_mut(9).unwrap();
    assert_eq!(queue.capacity(), QueueSize::limited(1));
    assert_eq!(queue.poll_mut().unwrap(), Some(9));
  }
}
