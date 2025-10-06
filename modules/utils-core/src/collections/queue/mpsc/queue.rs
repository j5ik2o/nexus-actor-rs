use super::traits::{MpscBackend, SharedMpscHandle};
use crate::collections::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

/// Queue facade that operates on a [`MpscBackend`].
#[derive(Debug)]
pub struct MpscQueue<S, T>
where
  S: SharedMpscHandle<T>, {
  storage: S,
  _marker: core::marker::PhantomData<T>,
}

impl<S, T> MpscQueue<S, T>
where
  S: SharedMpscHandle<T>,
{
  pub fn new(storage: S) -> Self {
    Self {
      storage,
      _marker: core::marker::PhantomData,
    }
  }

  pub fn storage(&self) -> &S {
    &self.storage
  }

  pub fn into_storage(self) -> S {
    self.storage
  }

  pub fn set_capacity(&self, capacity: Option<usize>) -> bool {
    self.storage.backend().set_capacity(capacity)
  }

  pub fn offer(&self, element: T) -> Result<(), QueueError<T>> {
    self.storage.backend().try_send(element)
  }

  pub fn poll(&self) -> Result<Option<T>, QueueError<T>> {
    self.storage.backend().try_recv()
  }

  pub fn clean_up(&self) {
    self.storage.backend().close();
  }

  pub fn capacity(&self) -> QueueSize {
    self.storage.backend().capacity()
  }

  pub fn is_closed(&self) -> bool {
    self.storage.backend().is_closed()
  }

  fn backend(&self) -> &S::Backend {
    self.storage.backend()
  }
}

impl<S, T> Clone for MpscQueue<S, T>
where
  S: SharedMpscHandle<T>,
{
  fn clone(&self) -> Self {
    Self {
      storage: self.storage.clone(),
      _marker: core::marker::PhantomData,
    }
  }
}

impl<S, T> QueueBase<T> for MpscQueue<S, T>
where
  S: SharedMpscHandle<T>,
{
  fn len(&self) -> QueueSize {
    self.backend().len()
  }

  fn capacity(&self) -> QueueSize {
    self.capacity()
  }
}

impl<S, T> QueueWriter<T> for MpscQueue<S, T>
where
  S: SharedMpscHandle<T>,
{
  fn offer_mut(&mut self, element: T) -> Result<(), QueueError<T>> {
    self.backend().try_send(element)
  }
}

impl<S, T> QueueReader<T> for MpscQueue<S, T>
where
  S: SharedMpscHandle<T>,
{
  fn poll_mut(&mut self) -> Result<Option<T>, QueueError<T>> {
    self.backend().try_recv()
  }

  fn clean_up_mut(&mut self) {
    self.backend().close();
  }
}

impl<S, T> crate::collections::queue::SharedQueue<T> for MpscQueue<S, T>
where
  S: SharedMpscHandle<T>,
{
  fn offer(&self, element: T) -> Result<(), QueueError<T>> {
    self.offer(element)
  }

  fn poll(&self) -> Result<Option<T>, QueueError<T>> {
    self.poll()
  }

  fn clean_up(&self) {
    self.clean_up();
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  use core::fmt;

  use crate::collections::queue::mpsc::backend::RingBufferBackend;
  use crate::collections::queue::mpsc::traits::SharedMpscHandle;
  use crate::collections::queue::mpsc::{MpscBuffer, MpscQueue};
  use crate::collections::QueueError;

  struct RcBackendHandle<T>(Rc<RingBufferBackend<RefCell<MpscBuffer<T>>>>);

  impl<T> RcBackendHandle<T> {
    fn new(capacity: Option<usize>) -> Self {
      let buffer = RefCell::new(MpscBuffer::new(capacity));
      let backend = RingBufferBackend::new(buffer);
      Self(Rc::new(backend))
    }
  }

  impl<T> Clone for RcBackendHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> fmt::Debug for RcBackendHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      f.debug_struct("RcBackendHandle").finish()
    }
  }

  impl<T> core::ops::Deref for RcBackendHandle<T> {
    type Target = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> crate::sync::Shared<RingBufferBackend<RefCell<MpscBuffer<T>>>> for RcBackendHandle<T> {}

  impl<T> SharedMpscHandle<T> for RcBackendHandle<T> {
    type Backend = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn backend(&self) -> &Self::Backend {
      &self.0
    }
  }

  #[test]
  fn buffer_offer_and_poll() {
    let mut buffer: MpscBuffer<u32> = MpscBuffer::new(Some(1));
    assert!(buffer.offer(1).is_ok());
    assert!(matches!(buffer.offer(2), Err(QueueError::Full(2))));
    assert_eq!(buffer.poll().unwrap(), Some(1));
    assert!(buffer.poll().unwrap().is_none());
    buffer.clean_up();
    assert!(matches!(buffer.offer(3), Err(QueueError::Closed(3))));
  }

  #[test]
  fn shared_queue_shared_operations() {
    let queue: MpscQueue<_, u32> = MpscQueue::new(RcBackendHandle::<u32>::new(Some(2)));
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert!(queue.offer(3).is_err());
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), Some(2));
  }

  #[test]
  fn shared_queue_cleanup_marks_closed() {
    let queue: MpscQueue<_, u32> = MpscQueue::new(RcBackendHandle::<u32>::new(None));
    queue.offer(1).unwrap();
    queue.clean_up();
    assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer(2), Err(QueueError::Closed(2))));
  }
}
