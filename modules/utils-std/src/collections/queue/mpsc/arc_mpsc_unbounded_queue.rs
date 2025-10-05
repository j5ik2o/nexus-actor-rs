use crate::collections::queue::mpsc::backend::TokioUnboundedMpscBackend;
use crate::sync::ArcShared;
use nexus_utils_core_rs::{
  Element, MpscBackend, MpscBuffer, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, RingBufferBackend,
  SharedMpscQueue, SharedQueue,
};
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ArcMpscUnboundedQueue<E> {
  inner: SharedMpscQueue<ArcShared<dyn MpscBackend<E> + Send + Sync>, E>,
}

impl<E> fmt::Debug for ArcMpscUnboundedQueue<E> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ArcMpscUnboundedQueue").finish()
  }
}

impl<E> ArcMpscUnboundedQueue<E>
where
  E: Element,
{
  pub fn new() -> Self {
    Self::with_tokio()
  }

  pub fn with_tokio() -> Self {
    Self::from_backend(TokioUnboundedMpscBackend::new())
  }

  pub fn with_ring_buffer() -> Self {
    let backend = RingBufferBackend::new(Mutex::new(MpscBuffer::new(None)));
    Self::from_backend(backend)
  }

  fn from_backend<B>(backend: B) -> Self
  where
    B: MpscBackend<E> + Send + Sync + 'static, {
    let arc_backend: Arc<dyn MpscBackend<E> + Send + Sync> = Arc::new(backend);
    let storage = ArcShared::from_arc(arc_backend);
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
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E: Element> QueueReader<E> for ArcMpscUnboundedQueue<E> {
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E: Element> SharedQueue<E> for ArcMpscUnboundedQueue<E> {
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&self) {
    self.clean_up_shared();
  }
}

impl<E> Default for ArcMpscUnboundedQueue<E>
where
  E: Element,
{
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

  #[test]
  fn unbounded_queue_ring_buffer_constructor() {
    let queue = ArcMpscUnboundedQueue::with_ring_buffer();
    queue.offer_shared(1).unwrap();
    assert_eq!(queue.poll_shared().unwrap(), Some(1));
  }
}
