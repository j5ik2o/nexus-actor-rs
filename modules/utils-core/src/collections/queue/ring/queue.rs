use core::marker::PhantomData;

use crate::collections::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedQueue};

use super::{handle::SharedQueueHandle, storage::QueueStorage};

#[derive(Debug)]
pub struct RingQueue<S, E>
where
  S: SharedQueueHandle<E>, {
  storage: S,
  _marker: PhantomData<E>,
}

impl<S, E> RingQueue<S, E>
where
  S: SharedQueueHandle<E>,
{
  pub fn new(storage: S) -> Self {
    Self {
      storage,
      _marker: PhantomData,
    }
  }

  pub fn storage(&self) -> &S {
    &self.storage
  }

  pub fn into_storage(self) -> S {
    self.storage
  }

  pub fn set_dynamic(&self, dynamic: bool) {
    self.storage.storage().with_write(|buffer| buffer.set_dynamic(dynamic));
  }

  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  pub fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.storage.storage().with_write(|buffer| buffer.offer_mut(element))
  }

  pub fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.storage.storage().with_write(|buffer| buffer.poll_mut())
  }

  pub fn clean_up(&self) {
    self.storage.storage().with_write(|buffer| buffer.clean_up_mut());
  }
}

impl<S, E> Clone for RingQueue<S, E>
where
  S: SharedQueueHandle<E>,
{
  fn clone(&self) -> Self {
    Self {
      storage: self.storage.clone(),
      _marker: PhantomData,
    }
  }
}

impl<S, E> QueueBase<E> for RingQueue<S, E>
where
  S: SharedQueueHandle<E>,
{
  fn len(&self) -> QueueSize {
    self.storage.storage().with_read(|buffer| buffer.len())
  }

  fn capacity(&self) -> QueueSize {
    self.storage.storage().with_read(|buffer| buffer.capacity())
  }
}

impl<S, E> QueueWriter<E> for RingQueue<S, E>
where
  S: SharedQueueHandle<E>,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }
}

impl<S, E> QueueReader<E> for RingQueue<S, E>
where
  S: SharedQueueHandle<E>,
{
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  fn clean_up_mut(&mut self) {
    self.clean_up();
  }
}

impl<S, E> SharedQueue<E> for RingQueue<S, E>
where
  S: SharedQueueHandle<E>,
{
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
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

  use super::super::RingBuffer;
  use super::{RingQueue, SharedQueueHandle};

  struct RcHandle<E>(Rc<RefCell<RingBuffer<E>>>);

  impl<E> Clone for RcHandle<E> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<E> core::ops::Deref for RcHandle<E> {
    type Target = RefCell<RingBuffer<E>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<E> SharedQueueHandle<E> for RcHandle<E> {
    type Storage = RefCell<RingBuffer<E>>;

    fn storage(&self) -> &Self::Storage {
      &self.0
    }
  }

  impl<E> crate::sync::Shared<RefCell<RingBuffer<E>>> for RcHandle<E> {}

  #[test]
  fn shared_ring_queue_offer_poll() {
    let storage = RcHandle(Rc::new(RefCell::new(RingBuffer::new(2))));
    let queue = RingQueue::new(storage);

    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), Some(2));
    assert_eq!(queue.poll().unwrap(), None);
  }
}
