use alloc::vec::Vec;

use super::{QueueError, QueueSize};
use crate::sync::Shared;

/// Stack-specific error type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StackError<T> {
  Full(T),
}

impl<T> From<StackError<T>> for QueueError<T> {
  fn from(err: StackError<T>) -> Self {
    match err {
      StackError::Full(value) => QueueError::Full(value),
    }
  }
}

/// Owning stack buffer that stores values in LIFO order.
#[derive(Debug, Clone)]
pub struct StackBuffer<T> {
  items: Vec<T>,
  capacity: Option<usize>,
}

impl<T> StackBuffer<T> {
  pub fn new() -> Self {
    Self {
      items: Vec::new(),
      capacity: None,
    }
  }

  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      items: Vec::with_capacity(capacity),
      capacity: Some(capacity),
    }
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
      if self.items.len() > limit {
        self.items.truncate(limit);
      }
    }
  }

  pub fn len(&self) -> QueueSize {
    QueueSize::limited(self.items.len())
  }

  pub fn is_empty(&self) -> bool {
    self.items.is_empty()
  }

  pub fn push(&mut self, value: T) -> Result<(), StackError<T>> {
    if matches!(self.capacity, Some(limit) if self.items.len() >= limit) {
      return Err(StackError::Full(value));
    }
    self.items.push(value);
    Ok(())
  }

  pub fn pop(&mut self) -> Option<T> {
    self.items.pop()
  }

  pub fn peek(&self) -> Option<&T> {
    self.items.last()
  }

  pub fn clear(&mut self) {
    self.items.clear();
  }
}

impl<T> Default for StackBuffer<T> {
  fn default() -> Self {
    Self::new()
  }
}

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

/// Shared stack facade that operates through a [`StackStorage`].
#[derive(Debug)]
pub struct SharedStack<S, T>
where
  S: SharedStackHandle<T>, {
  storage: S,
  _marker: core::marker::PhantomData<T>,
}

impl<S, T> SharedStack<S, T>
where
  S: SharedStackHandle<T>,
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

  pub fn push_shared(&self, value: T) -> Result<(), StackError<T>> {
    self.storage.storage().with_write(|buffer| buffer.push(value))
  }

  pub fn pop_shared(&self) -> Option<T> {
    self.storage.storage().with_write(|buffer| buffer.pop())
  }

  pub fn peek_shared(&self) -> Option<T>
  where
    T: Clone, {
    self.storage.storage().with_read(|buffer| buffer.peek().cloned())
  }

  pub fn clear_shared(&self) {
    self.storage.storage().with_write(|buffer| buffer.clear());
  }

  pub fn len_shared(&self) -> QueueSize {
    self.storage.storage().with_read(|buffer| buffer.len())
  }

  pub fn capacity_shared(&self) -> QueueSize {
    self.storage.storage().with_read(|buffer| buffer.capacity())
  }
}

impl<S, T> Clone for SharedStack<S, T>
where
  S: SharedStackHandle<T>,
{
  fn clone(&self) -> Self {
    Self {
      storage: self.storage.clone(),
      _marker: core::marker::PhantomData,
    }
  }
}

impl<S, T> StackBase<T> for SharedStack<S, T>
where
  S: SharedStackHandle<T>,
{
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    self.capacity_shared()
  }
}

impl<S, T> StackMut<T> for SharedStack<S, T>
where
  S: SharedStackHandle<T>,
{
  fn push(&mut self, value: T) -> Result<(), StackError<T>> {
    SharedStack::push_shared(self, value)
  }

  fn pop(&mut self) -> Option<T> {
    SharedStack::pop_shared(self)
  }

  fn clear(&mut self) {
    SharedStack::clear_shared(self);
  }

  fn peek(&self) -> Option<T>
  where
    T: Clone, {
    SharedStack::peek_shared(self)
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
  use super::{StackBuffer, StackStorage};
  use core::cell::RefCell;

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
  use super::{StackBuffer, StackStorage};
  use std::sync::Mutex;

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

#[cfg(test)]
mod tests {
  extern crate alloc;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  use super::*;

  #[derive(Debug)]
  struct RcStorage<T>(Rc<RefCell<StackBuffer<T>>>);

  impl<T> RcStorage<T> {
    fn new() -> Self {
      Self(Rc::new(RefCell::new(StackBuffer::new())))
    }
  }

  impl<T> Clone for RcStorage<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> core::ops::Deref for RcStorage<T> {
    type Target = RefCell<StackBuffer<T>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> Shared<RefCell<StackBuffer<T>>> for RcStorage<T> {}

  impl<T> SharedStackHandle<T> for RcStorage<T> {
    type Storage = RefCell<StackBuffer<T>>;

    fn storage(&self) -> &Self::Storage {
      &self.0
    }
  }

  #[test]
  fn stack_buffer_push_pop() {
    let mut buffer = StackBuffer::with_capacity(1);
    assert!(buffer.push(1).is_ok());
    assert!(matches!(buffer.push(2), Err(StackError::Full(2))));
    assert_eq!(buffer.pop(), Some(1));
    assert!(buffer.pop().is_none());
  }

  #[test]
  fn shared_stack_push_pop() {
    let storage = RcStorage::new();
    let mut stack = SharedStack::new(storage.clone());
    stack.set_capacity(Some(2));

    stack.push(1).unwrap();
    stack.push(2).unwrap();
    assert!(stack.push(3).is_err());

    assert_eq!(stack.pop(), Some(2));
    assert_eq!(stack.pop(), Some(1));
    assert!(stack.pop().is_none());
  }

  #[test]
  fn shared_stack_clone_observes_state() {
    let stack = SharedStack::new(RcStorage::new());
    stack.push_shared(10).unwrap();
    let cloned = stack.clone();
    cloned.push_shared(11).unwrap();
    assert_eq!(stack.len_shared().to_usize(), 2);
    assert_eq!(cloned.pop_shared(), Some(11));
    assert_eq!(stack.pop_shared(), Some(10));
  }

  #[test]
  fn shared_stack_peek() {
    let stack = SharedStack::new(RcStorage::new());
    stack.push_shared(7).unwrap();
    assert_eq!(stack.peek_shared(), Some(7));
    stack.pop_shared();
    assert_eq!(stack.peek_shared(), None);
  }
}
