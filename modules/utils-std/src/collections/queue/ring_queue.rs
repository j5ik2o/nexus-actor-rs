use std::sync::{Arc, Mutex};

use nexus_utils_core_rs::{
  collections::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, RingBuffer},
  DEFAULT_CAPACITY,
};

#[derive(Debug, Clone)]
pub struct RingQueue<E> {
  inner: Arc<Mutex<RingBuffer<E>>>,
}

impl<E> RingQueue<E> {
  pub fn new(capacity: usize) -> Self {
    Self {
      inner: Arc::new(Mutex::new(RingBuffer::new(capacity))),
    }
  }

  pub fn with_dynamic(mut self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  pub fn set_dynamic(&mut self, dynamic: bool) {
    self.with_lock(|buffer| buffer.set_dynamic(dynamic));
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.with_lock(|buffer| buffer.offer(element))
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.with_lock(|buffer| buffer.poll())
  }

  pub fn clean_up_shared(&self) {
    self.with_lock(|buffer| buffer.clean_up());
  }

  pub fn len_shared(&self) -> QueueSize {
    self.with_lock(|buffer| buffer.len())
  }

  fn with_lock<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R {
    let mut guard = self.inner.lock().expect("ring queue mutex poisoned");
    f(&mut guard)
  }
}

impl<E> Default for RingQueue<E> {
  fn default() -> Self {
    Self::new(DEFAULT_CAPACITY)
  }
}

impl<E> QueueBase<E> for RingQueue<E> {
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    self.with_lock(|buffer| buffer.capacity())
  }
}

impl<E> QueueWriter<E> for RingQueue<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E> QueueReader<E> for RingQueue<E> {
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
  fn ring_queue_offer_poll() {
    let queue = RingQueue::new(2).with_dynamic(false);
    queue.offer_shared(1).unwrap();
    queue.offer_shared(2).unwrap();
    assert_eq!(queue.offer_shared(3), Err(QueueError::Full(3)));

    assert_eq!(queue.poll_shared().unwrap(), Some(1));
    assert_eq!(queue.poll_shared().unwrap(), Some(2));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }
}
