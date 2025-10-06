use core::cell::RefCell;

use nexus_utils_core_rs::{
  QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter, RingBuffer, RingQueue, DEFAULT_CAPACITY,
};

use crate::sync::RcShared;

#[derive(Debug, Clone)]
pub struct RcRingQueue<E> {
  inner: RingQueue<RcShared<RefCell<RingBuffer<E>>>, E>,
}

impl<E> RcRingQueue<E> {
  pub fn new(capacity: usize) -> Self {
    let storage = RcShared::new(RefCell::new(RingBuffer::new(capacity)));
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

impl<E> Default for RcRingQueue<E> {
  fn default() -> Self {
    Self::new(DEFAULT_CAPACITY)
  }
}

impl<E> QueueBase<E> for RcRingQueue<E> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E> QueueWriter<E> for RcRingQueue<E> {
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E> QueueReader<E> for RcRingQueue<E> {
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E> QueueRw<E> for RcRingQueue<E> {
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
  fn rc_ring_queue_offer_poll() {
    let queue = RcRingQueue::new(1).with_dynamic(false);
    queue.offer(10).unwrap();
    assert_eq!(queue.poll().unwrap(), Some(10));
    assert_eq!(queue.poll().unwrap(), None);
  }

  #[test]
  fn rc_ring_queue_shared_clone() {
    let queue = RcRingQueue::new(4);
    let cloned = queue.clone();

    queue.offer(1).unwrap();
    cloned.offer(2).unwrap();

    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(cloned.poll().unwrap(), Some(2));
  }

  #[test]
  fn rc_ring_queue_clean_up_resets_state() {
    let queue = RcRingQueue::new(2);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();

    queue.clean_up();
    assert_eq!(queue.len().to_usize(), 0);
    assert!(queue.poll().unwrap().is_none());
  }

  #[test]
  fn rc_ring_queue_dynamic_growth() {
    let queue = RcRingQueue::new(1).with_dynamic(true);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert_eq!(queue.len().to_usize(), 2);
  }

  #[test]
  fn rc_ring_queue_set_dynamic_switches_mode() {
    let queue = RcRingQueue::new(1);
    queue.set_dynamic(false);
    queue.offer(1).unwrap();
    assert!(matches!(queue.offer(2), Err(QueueError::Full(2))));
  }

  #[test]
  fn rc_ring_queue_trait_interface() {
    let mut queue = RcRingQueue::new(1).with_dynamic(false);
    queue.offer_mut(3).unwrap();
    assert_eq!(queue.poll_mut().unwrap(), Some(3));
  }
}
