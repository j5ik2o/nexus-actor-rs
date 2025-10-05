use crate::collections::stack::buffer::StackBuffer;
use crate::collections::QueueSize;
use crate::sync::Shared;

use super::StackError;

/// Abstraction over storage used by [`SharedStack`].
pub trait StackStorage<T> {
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R;
  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R;
}

/// Shared pointer handle that exposes the storage implementation.
pub trait SharedStackHandle<T>: Shared<Self::Storage> + Clone {
  type Storage: StackStorage<T> + ?Sized;

  fn storage(&self) -> &Self::Storage;
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
