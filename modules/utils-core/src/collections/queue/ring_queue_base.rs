use core::fmt::Debug;

pub const DEFAULT_CAPACITY: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueSize(usize);

impl QueueSize {
  pub const fn new(value: usize) -> Self {
    Self(value)
  }

  pub const fn to_usize(self) -> usize {
    self.0
  }
}

impl Default for QueueSize {
  fn default() -> Self {
    Self::new(0)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueError<T> {
  Full(T),
}

pub trait QueueBase<E> {
  fn len(&self) -> QueueSize;
  fn capacity(&self) -> QueueSize;
}

pub trait QueueWriter<E>: QueueBase<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>>;
}

pub trait QueueReader<E>: QueueBase<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>>;
  fn clean_up(&mut self);
}
