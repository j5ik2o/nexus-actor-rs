use crate::collections::queue::mpsc::backend::TokioBoundedMpscBackend;
use crate::sync::ArcShared;
use nexus_utils_core_rs::{
  Element, MpscBackend, MpscBuffer, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, RingBufferBackend,
  SharedMpscQueue, SharedQueue,
};
use std::fmt;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct ArcMpscBoundedQueue<E> {
  inner: SharedMpscQueue<ArcShared<dyn MpscBackend<E> + Send + Sync>, E>,
}

impl<E> fmt::Debug for ArcMpscBoundedQueue<E> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ArcMpscBoundedQueue").finish()
  }
}

impl<E> ArcMpscBoundedQueue<E>
where
  E: Element,
{
  pub fn new(capacity: usize) -> Self {
    Self::with_tokio(capacity)
  }

  pub fn with_tokio(capacity: usize) -> Self {
    Self::from_backend(TokioBoundedMpscBackend::new(capacity))
  }

  pub fn with_ring_buffer(capacity: usize) -> Self {
    let backend = RingBufferBackend::new(Mutex::new(MpscBuffer::new(Some(capacity))));
    Self::from_backend(backend)
  }

  fn from_backend<B>(backend: B) -> Self
  where
    B: MpscBackend<E> + Send + Sync + 'static,
  {
    let arc_backend: Arc<dyn MpscBackend<E> + Send + Sync> = Arc::new(backend);
    let storage = ArcShared::from_arc(arc_backend);
    Self {
      inner: SharedMpscQueue::new(storage),
    }
  }
}

impl<E: Element> QueueBase<E> for ArcMpscBoundedQueue<E> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E: Element> QueueWriter<E> for ArcMpscBoundedQueue<E> {
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E: Element> QueueReader<E> for ArcMpscBoundedQueue<E> {
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E: Element> SharedQueue<E> for ArcMpscBoundedQueue<E> {
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
  fn bounded_queue_respects_capacity() {
    let queue = ArcMpscBoundedQueue::new(1);
    queue.offer(1).unwrap();
    let err = queue.offer(2).unwrap_err();
    assert!(matches!(err, QueueError::Full(2)));
  }

  #[test]
  fn bounded_queue_poll_returns_items() {
    let queue = ArcMpscBoundedQueue::new(2);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();

    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), Some(2));
    assert_eq!(queue.poll().unwrap(), None);
  }

  #[test]
  fn bounded_queue_clean_up_closes_channel() {
    let queue = ArcMpscBoundedQueue::new(1);
    queue.offer(1).unwrap();
    queue.clean_up();

    assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer(2), Err(QueueError::Closed(2))));
  }

  #[test]
  fn bounded_queue_ring_buffer_constructor() {
    let queue = ArcMpscBoundedQueue::with_ring_buffer(1);
    queue.offer(1).unwrap();
    assert!(matches!(queue.offer(2), Err(QueueError::Full(2))));
  }
}
