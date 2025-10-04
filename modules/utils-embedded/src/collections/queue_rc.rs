use alloc::rc::Rc;
use core::cell::RefCell;

use nexus_utils_core_rs::{
  collections::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, RingBuffer},
  DEFAULT_CAPACITY,
};

#[derive(Debug, Clone)]
pub struct RcRingQueue<E> {
  inner: Rc<RefCell<RingBuffer<E>>>,
}

impl<E> RcRingQueue<E> {
  pub fn new(capacity: usize) -> Self {
    Self {
      inner: Rc::new(RefCell::new(RingBuffer::new(capacity))),
    }
  }

  pub fn with_dynamic(mut self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  pub fn set_dynamic(&mut self, dynamic: bool) {
    self.inner.borrow_mut().set_dynamic(dynamic);
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.inner.borrow_mut().offer(element)
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.inner.borrow_mut().poll()
  }

  pub fn len_shared(&self) -> QueueSize {
    self.inner.borrow().len()
  }

  pub fn clean_up_shared(&self) {
    self.inner.borrow_mut().clean_up();
  }
}

impl<E> Default for RcRingQueue<E> {
  fn default() -> Self {
    Self::new(DEFAULT_CAPACITY)
  }
}

impl<E> QueueBase<E> for RcRingQueue<E> {
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.borrow().capacity()
  }
}

impl<E> QueueWriter<E> for RcRingQueue<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E> QueueReader<E> for RcRingQueue<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&mut self) {
    self.clean_up_shared();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rc_ring_queue_offer_poll() {
    let queue = RcRingQueue::new(1);
    queue.offer_shared(10).unwrap();
    assert_eq!(queue.poll_shared().unwrap(), Some(10));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }
}
