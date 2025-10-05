use alloc::collections::VecDeque;

use super::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};
use crate::sync::Shared;

#[derive(Debug, Clone)]
pub struct MpscBuffer<T> {
  buffer: VecDeque<T>,
  capacity: Option<usize>,
  closed: bool,
}

impl<T> MpscBuffer<T> {
  pub fn new(capacity: Option<usize>) -> Self {
    Self {
      buffer: VecDeque::new(),
      capacity,
      closed: false,
    }
  }

  pub fn len(&self) -> QueueSize {
    QueueSize::limited(self.buffer.len())
  }

  pub fn capacity(&self) -> QueueSize {
    match self.capacity {
      Some(limit) => QueueSize::limited(limit),
      None => QueueSize::limitless(),
    }
  }

  pub fn set_capacity(&mut self, capacity: Option<usize>) {
    self.capacity = capacity;
    if let Some(limit) = capacity {
      if self.buffer.len() > limit {
        self.buffer.truncate(limit);
      }
    }
  }

  pub fn offer(&mut self, element: T) -> Result<(), QueueError<T>> {
    if self.closed {
      return Err(QueueError::Closed(element));
    }
    if matches!(self.capacity, Some(limit) if self.buffer.len() >= limit) {
      return Err(QueueError::Full(element));
    }
    self.buffer.push_back(element);
    Ok(())
  }

  pub fn poll(&mut self) -> Result<Option<T>, QueueError<T>> {
    if let Some(item) = self.buffer.pop_front() {
      return Ok(Some(item));
    }
    if self.closed {
      Err(QueueError::Disconnected)
    } else {
      Ok(None)
    }
  }

  pub fn clean_up(&mut self) {
    self.buffer.clear();
    self.closed = true;
  }

  pub fn is_closed(&self) -> bool {
    self.closed
  }

  pub fn mark_closed(&mut self) {
    self.closed = true;
  }
}

impl<T> Default for MpscBuffer<T> {
  fn default() -> Self {
    Self::new(None)
  }
}

pub trait MpscStorage<T> {
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R;
  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R;
}

pub trait SharedMpscHandle<T>: Shared<Self::Storage> + Clone {
  type Storage: MpscStorage<T> + ?Sized;

  fn storage(&self) -> &Self::Storage;
}

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

#[cfg(feature = "alloc")]
mod alloc_impls {
  use core::cell::RefCell;

  use super::{MpscBuffer, MpscStorage};

  impl<T> MpscStorage<T> for RefCell<MpscBuffer<T>> {
    fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
      f(&self.borrow())
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
      f(&mut self.borrow_mut())
    }
  }
}

#[cfg(all(feature = "alloc", feature = "std"))]
mod std_impls {
  use std::sync::Mutex;

  use super::{MpscBuffer, MpscStorage};

  impl<T> MpscStorage<T> for Mutex<MpscBuffer<T>> {
    fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
      let guard = self.lock().expect("mutex poisoned");
      f(&guard)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
      let mut guard = self.lock().expect("mutex poisoned");
      f(&mut guard)
    }
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;
  use super::*;
  use alloc::rc::Rc;
  use core::cell::RefCell;

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

  impl<T> Shared<RefCell<MpscBuffer<T>>> for RcHandle<T> {}

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
    let queue: SharedMpscQueue<_, _> = SharedMpscQueue::new(RcHandle::new(Some(2)));
    queue.offer_shared(1).unwrap();
    queue.offer_shared(2).unwrap();
    assert!(queue.offer_shared(3).is_err());
    assert_eq!(queue.poll_shared().unwrap(), Some(1));
    assert_eq!(queue.poll_shared().unwrap(), Some(2));
  }

  #[test]
  fn shared_queue_cleanup_marks_closed() {
    let queue: SharedMpscQueue<_, _> = SharedMpscQueue::new(RcHandle::new(None));
    queue.offer_shared(1).unwrap();
    queue.clean_up_shared();
    assert!(matches!(queue.poll_shared(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer_shared(2), Err(QueueError::Closed(2))));
  }
}
