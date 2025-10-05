use crate::sync::ArcShared;
use nexus_utils_core_rs::{
  Element, MpscBuffer, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedMpscQueue, SharedQueue,
};
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ArcMpscUnboundedQueue<E> {
  inner: SharedMpscQueue<ArcShared<Mutex<MpscBuffer<E>>>, E>,
}

impl<E> ArcMpscUnboundedQueue<E> {
  pub fn new() -> Self {
    let storage = ArcShared::new(Mutex::new(MpscBuffer::new(None)));
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

impl<E: Element> QueueBase<E> for ArcMpscUnboundedQueue<E> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E: Element> QueueWriter<E> for ArcMpscUnboundedQueue<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }
}

impl<E: Element> QueueReader<E> for ArcMpscUnboundedQueue<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  fn clean_up(&mut self) {
    self.inner.clean_up();
  }
}

impl<E: Element> SharedQueue<E> for ArcMpscUnboundedQueue<E> {
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

impl<E> Default for ArcMpscUnboundedQueue<E> {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn unbounded_queue_offer_poll_cycle() {
    let queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    queue.offer_shared(10).unwrap();
    queue.offer_shared(20).unwrap();

    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll_shared().unwrap(), Some(10));
    assert_eq!(queue.poll_shared().unwrap(), Some(20));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }

  #[test]
  fn unbounded_queue_closed_state() {
    let queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    queue.offer_shared(1).unwrap();
    queue.clean_up_shared();
    assert!(matches!(queue.poll_shared(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer_shared(2), Err(QueueError::Closed(2))));
  }
}
