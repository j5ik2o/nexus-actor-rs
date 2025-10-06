use core::cell::RefCell;

use nexus_utils_core_rs::{
  Element, MpscBuffer, MpscQueue, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter,
  RingBufferBackend,
};

use crate::sync::RcShared;

#[derive(Debug, Clone)]
pub struct RcMpscUnboundedQueue<E> {
  inner: MpscQueue<RcShared<RingBufferBackend<RefCell<MpscBuffer<E>>>>, E>,
}

impl<E> RcMpscUnboundedQueue<E> {
  pub fn new() -> Self {
    let storage = RcShared::new(RingBufferBackend::new(RefCell::new(MpscBuffer::new(None))));
    Self {
      inner: MpscQueue::new(storage),
    }
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
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E: Element> QueueReader<E> for RcMpscUnboundedQueue<E> {
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E: Element> QueueRw<E> for RcMpscUnboundedQueue<E> {
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
  use nexus_utils_core_rs::{QueueBase, QueueRw};

  #[test]
  fn rc_unbounded_offer_poll() {
    let queue: RcMpscUnboundedQueue<u32> = RcMpscUnboundedQueue::new();
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), Some(2));
    assert_eq!(queue.poll().unwrap(), None);
  }

  #[test]
  fn rc_unbounded_clean_up_signals_disconnected() {
    let queue: RcMpscUnboundedQueue<u8> = RcMpscUnboundedQueue::new();
    queue.offer(1).unwrap();
    queue.clean_up();

    assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer(2), Err(QueueError::Closed(2))));
  }

  #[test]
  fn rc_unbounded_offer_poll_via_traits() {
    let mut queue: RcMpscUnboundedQueue<u32> = RcMpscUnboundedQueue::new();
    queue.offer_mut(1).unwrap();
    assert_eq!(queue.poll_mut().unwrap(), Some(1));
  }
}
