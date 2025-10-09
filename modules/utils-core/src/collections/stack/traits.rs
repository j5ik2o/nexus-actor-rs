use crate::collections::stack::buffer::StackBuffer;
use crate::collections::QueueSize;
use crate::sync::Shared;

use super::StackError;

/// Abstraction for storage used by stack backends.
///
/// This trait provides access to the internal data structure of a stack,
/// enabling both read-only and writable operations.
pub trait StackStorage<T> {
  /// Executes a closure with read-only access.
  ///
  /// # Arguments
  ///
  /// * `f` - Closure that receives an immutable reference to the stack buffer
  ///
  /// # Returns
  ///
  /// Result of executing the closure
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R;

  /// Executes a closure with writable access.
  ///
  /// # Arguments
  ///
  /// * `f` - Closure that receives a mutable reference to the stack buffer
  ///
  /// # Returns
  ///
  /// Result of executing the closure
  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R;
}

/// Backend abstraction for stack operations.
///
/// This trait defines basic stack operations (push, pop, peek, etc.),
/// providing an interface independent of the concrete storage implementation.
pub trait StackBackend<T> {
  /// Pushes a value onto the stack.
  ///
  /// # Arguments
  ///
  /// * `value` - Value to push
  ///
  /// # Returns
  ///
  /// * `Ok(())` - On success
  /// * `Err(StackError<T>)` - If capacity limit is reached
  fn push(&self, value: T) -> Result<(), StackError<T>>;

  /// Pops a value from the stack.
  ///
  /// # Returns
  ///
  /// * `Some(T)` - Last value if stack is not empty
  /// * `None` - If stack is empty
  fn pop(&self) -> Option<T>;

  /// Clears all elements from the stack.
  fn clear(&self);

  /// Gets the current number of elements in the stack.
  ///
  /// # Returns
  ///
  /// `QueueSize` representing the element count
  fn len(&self) -> QueueSize;

  /// Gets the capacity of the stack.
  ///
  /// # Returns
  ///
  /// `QueueSize` representing capacity (`QueueSize::Unlimited` if unlimited)
  fn capacity(&self) -> QueueSize;

  /// Sets the stack's capacity.
  ///
  /// # Arguments
  ///
  /// * `capacity` - New capacity (`None` for unlimited)
  fn set_capacity(&self, capacity: Option<usize>);

  /// Checks if the stack is empty.
  ///
  /// # Returns
  ///
  /// * `true` - If stack is empty
  /// * `false` - If stack contains elements
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }

  /// Gets the top value of the stack without popping.
  ///
  /// # Returns
  ///
  /// * `Some(T)` - Clone of the top value if stack is not empty
  /// * `None` - If stack is empty
  fn peek(&self) -> Option<T>
  where
    T: Clone;
}

/// Handle that exposes a [`StackBackend`].
///
/// This trait provides shared access to a stack backend,
/// allowing multiple owners to use the same stack instance.
pub trait StackHandle<T>: Shared<Self::Backend> + Clone {
  /// Backend type managed by this handle.
  type Backend: StackBackend<T> + ?Sized;

  /// Gets a reference to the backend.
  ///
  /// # Returns
  ///
  /// Reference to the stack backend
  fn backend(&self) -> &Self::Backend;
}

/// Backend implementation that operates directly on [`StackStorage`].
///
/// This backend implements stack operations using the storage abstraction.
#[derive(Debug)]
pub struct StackStorageBackend<S> {
  storage: S,
}

impl<S> StackStorageBackend<S> {
  /// Creates a new `StackStorageBackend` with the specified storage.
  ///
  /// # Arguments
  ///
  /// * `storage` - Storage implementation to use
  pub const fn new(storage: S) -> Self {
    Self { storage }
  }

  /// Gets a reference to the storage.
  ///
  /// # Returns
  ///
  /// Reference to the internal storage
  pub fn storage(&self) -> &S {
    &self.storage
  }

  /// Consumes the backend and extracts the storage.
  ///
  /// # Returns
  ///
  /// Ownership of the internal storage
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
    T: Clone, {
    self.storage.with_read(|buffer| buffer.peek().cloned())
  }
}

/// Base trait for stack-like collections.
///
/// This trait defines basic information retrieval methods for stacks.
pub trait StackBase<T> {
  /// Gets the current number of elements in the stack.
  ///
  /// # Returns
  ///
  /// `QueueSize` representing the element count
  fn len(&self) -> QueueSize;

  /// Gets the capacity of the stack.
  ///
  /// # Returns
  ///
  /// `QueueSize` representing capacity (`QueueSize::Unlimited` if unlimited)
  fn capacity(&self) -> QueueSize;

  /// Checks if the stack is empty.
  ///
  /// # Returns
  ///
  /// * `true` - If stack is empty
  /// * `false` - If stack contains elements
  fn is_empty(&self) -> bool {
    self.len().to_usize() == 0
  }
}

/// Mutable stack interface.
///
/// This trait provides stack modification operations (push, pop, clear, etc.).
pub trait StackMut<T>: StackBase<T> {
  /// Pushes a value onto the stack.
  ///
  /// # Arguments
  ///
  /// * `value` - Value to push
  ///
  /// # Returns
  ///
  /// * `Ok(())` - On success
  /// * `Err(StackError<T>)` - If capacity limit is reached
  fn push(&mut self, value: T) -> Result<(), StackError<T>>;

  /// Pops a value from the stack.
  ///
  /// # Returns
  ///
  /// * `Some(T)` - Last value if stack is not empty
  /// * `None` - If stack is empty
  fn pop(&mut self) -> Option<T>;

  /// Clears all elements from the stack.
  fn clear(&mut self);

  /// Gets the top value of the stack without popping.
  ///
  /// # Returns
  ///
  /// * `Some(T)` - Clone of the top value if stack is not empty
  /// * `None` - If stack is empty
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
