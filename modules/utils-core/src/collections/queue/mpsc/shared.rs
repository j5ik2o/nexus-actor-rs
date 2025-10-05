use super::traits::{MpscStorage, SharedMpscHandle};
use crate::collections::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

/// Shared queue facade that operates on a [`MpscStorage`].
#[derive(Debug)]
pub struct SharedMpscQueue<S, T>
where
  S: SharedMpscHandle<T>, {
  storage: S,
  _marker: core::marker::PhantomData<T>,
}

impl<S, T> SharedMpscQueue<S, T>
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

  pub fn set_capacity(&self, capacity: Option<usize>) {
    self
      .storage
      .storage()
      .with_write(|buffer| buffer.set_capacity(capacity));
  }

  pub fn offer_shared(&self, element: T) -> Result<(), QueueError<T>> {
    self.storage.storage().with_write(|buffer| buffer.offer(element))
  }

  pub fn poll_shared(&self) -> Result<Option<T>, QueueError<T>> {
    self.storage.storage().with_write(|buffer| buffer.poll())
  }

  pub fn clean_up_shared(&self) {
    self.storage.storage().with_write(|buffer| buffer.clean_up());
  }

  pub fn len_shared(&self) -> QueueSize {
    self.storage.storage().with_read(|buffer| buffer.len())
  }

  pub fn capacity_shared(&self) -> QueueSize {
    self.storage.storage().with_read(|buffer| buffer.capacity())
  }

  pub fn is_closed_shared(&self) -> bool {
    self.storage.storage().with_read(|buffer| buffer.is_closed())
  }
}

impl<S, T> Clone for SharedMpscQueue<S, T>
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

impl<S, T> QueueBase<T> for SharedMpscQueue<S, T>
where
  S: SharedMpscHandle<T>,
{
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    self.capacity_shared()
  }
}

impl<S, T> QueueWriter<T> for SharedMpscQueue<S, T>
where
  S: SharedMpscHandle<T>,
{
  fn offer(&mut self, element: T) -> Result<(), QueueError<T>> {
    SharedMpscQueue::offer_shared(self, element)
  }
}

impl<S, T> QueueReader<T> for SharedMpscQueue<S, T>
where
  S: SharedMpscHandle<T>,
{
  fn poll(&mut self) -> Result<Option<T>, QueueError<T>> {
    SharedMpscQueue::poll_shared(self)
  }

  fn clean_up(&mut self) {
    SharedMpscQueue::clean_up_shared(self);
  }
}

impl<S, T> crate::collections::queue::SharedQueue<T> for SharedMpscQueue<S, T>
where
  S: SharedMpscHandle<T>,
{
  fn offer_shared(&self, element: T) -> Result<(), QueueError<T>> {
    SharedMpscQueue::offer_shared(self, element)
  }

  fn poll_shared(&self) -> Result<Option<T>, QueueError<T>> {
    SharedMpscQueue::poll_shared(self)
  }

  fn clean_up_shared(&self) {
    SharedMpscQueue::clean_up_shared(self)
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  use crate::collections::queue::mpsc::buffer::MpscBuffer;
  use crate::collections::queue::mpsc::traits::SharedMpscHandle;
  use crate::collections::QueueError;
  use crate::collections::queue::mpsc::SharedMpscQueue;

  #[derive(Debug)]
  struct RcHandle<T>(Rc<RefCell<MpscBuffer<T>>>);

  impl<T> RcHandle<T> {
    fn new(capacity: Option<usize>) -> Self {
      Self(Rc::new(RefCell::new(MpscBuffer::new(capacity))))
    }
  }

  impl<T> Clone for RcHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> core::ops::Deref for RcHandle<T> {
    type Target = RefCell<MpscBuffer<T>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> crate::sync::Shared<RefCell<MpscBuffer<T>>> for RcHandle<T> {}

  impl<T> SharedMpscHandle<T> for RcHandle<T> {
    type Storage = RefCell<MpscBuffer<T>>;

    fn storage(&self) -> &Self::Storage {
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
    let queue: SharedMpscQueue<_, u32> = SharedMpscQueue::new(RcHandle::<u32>::new(Some(2)));
    queue.offer_shared(1).unwrap();
    queue.offer_shared(2).unwrap();
    assert!(queue.offer_shared(3).is_err());
    assert_eq!(queue.poll_shared().unwrap(), Some(1));
    assert_eq!(queue.poll_shared().unwrap(), Some(2));
  }

  #[test]
  fn shared_queue_cleanup_marks_closed() {
    let queue: SharedMpscQueue<_, u32> = SharedMpscQueue::new(RcHandle::<u32>::new(None));
    queue.offer_shared(1).unwrap();
    queue.clean_up_shared();
    assert!(matches!(queue.poll_shared(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer_shared(2), Err(QueueError::Closed(2))));
  }
}
