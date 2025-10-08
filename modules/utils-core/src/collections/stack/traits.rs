use crate::collections::stack::buffer::StackBuffer;
use crate::collections::QueueSize;
use crate::sync::Shared;

use super::StackError;

/// Abstraction over storage used by stack backends.
pub trait StackStorage<T> {
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R;
  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R;
}

/// Backend abstraction for stack operations.
pub trait StackBackend<T> {
  fn push(&self, value: T) -> Result<(), StackError<T>>;
  fn pop(&self) -> Option<T>;
  fn clear(&self);
  fn len(&self) -> QueueSize;
  fn capacity(&self) -> QueueSize;
  fn set_capacity(&self, capacity: Option<usize>);

  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }

  fn peek(&self) -> Option<T>
  where
    T: Clone;
}

/// Handle that exposes a [`StackBackend`].
pub trait StackHandle<T>: Shared<Self::Backend> + Clone {
  type Backend: StackBackend<T> + ?Sized;

  fn backend(&self) -> &Self::Backend;
}

/// Backend implementation that operates directly on a [`StackStorage`].
#[derive(Debug)]
pub struct StackStorageBackend<S> {
  storage: S,
}

impl<S> StackStorageBackend<S> {
  pub const fn new(storage: S) -> Self {
    Self { storage }
  }

  pub fn storage(&self) -> &S {
    &self.storage
  }

  pub fn into_storage(self) -> S {
    self.storage
  }
}

impl<S, T> StackBackend<T> for StackStorageBackend<S>
where
  S: StackStorage<T>,
{
  fn push(&self, value: T) -> Result<(), StackError<T>> {
    self.storage.with_write(|buffer| buffer.push(value))
  }

  fn pop(&self) -> Option<T> {
    self.storage.with_write(|buffer| buffer.pop())
  }

  fn clear(&self) {
    self.storage.with_write(|buffer| buffer.clear());
  }

  fn len(&self) -> QueueSize {
    self.storage.with_read(|buffer| buffer.len())
  }

  fn capacity(&self) -> QueueSize {
    self.storage.with_read(|buffer| buffer.capacity())
  }

  fn set_capacity(&self, capacity: Option<usize>) {
    self.storage.with_write(|buffer| buffer.set_capacity(capacity));
  }

  fn peek(&self) -> Option<T>
  where
    T: Clone,
  {
    self.storage.with_read(|buffer| buffer.peek().cloned())
  }
}

/// Base trait for stack-like collections.
pub trait StackBase<T> {
  fn len(&self) -> QueueSize;
  fn capacity(&self) -> QueueSize;

  fn is_empty(&self) -> bool {
    self.len().to_usize() == 0
  }
}

/// Mutable stack interface.
pub trait StackMut<T>: StackBase<T> {
  fn push(&mut self, value: T) -> Result<(), StackError<T>>;
  fn pop(&mut self) -> Option<T>;
  fn clear(&mut self);
  fn peek(&self) -> Option<T>
  where
    T: Clone;
}

#[cfg(feature = "alloc")]
mod alloc_impls {
  use core::cell::RefCell;

  use super::{StackBuffer, StackStorage};

  impl<T> StackStorage<T> for RefCell<StackBuffer<T>> {
    fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
      f(&self.borrow())
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
      f(&mut self.borrow_mut())
    }
  }
}

#[cfg(all(feature = "alloc", feature = "std"))]
mod std_impls {
  use std::sync::Mutex;

  use super::{StackBuffer, StackStorage};

  impl<T> StackStorage<T> for Mutex<StackBuffer<T>> {
    fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
      let guard = self.lock().expect("mutex poisoned");
      f(&guard)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
      let mut guard = self.lock().expect("mutex poisoned");
      f(&mut guard)
    }
  }
}
