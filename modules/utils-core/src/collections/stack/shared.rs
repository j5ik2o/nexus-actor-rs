use super::traits::{SharedStackHandle, StackBase, StackMut, StackStorage};
use crate::collections::QueueSize;

use super::StackError;

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

#[cfg(test)]
mod tests {
  extern crate alloc;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  use super::*;
  use crate::collections::stack::buffer::StackBuffer;
  use crate::collections::stack::traits::SharedStackHandle;
  use crate::sync::Shared;

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
  fn shared_stack_push_pop() {
    let stack = SharedStack::new(RcStorage::new());
    stack.set_capacity(Some(2));

    let mut clone = stack.clone();
    clone.push(1).unwrap();
    clone.push(2).unwrap();
    assert!(clone.push(3).is_err());

    assert_eq!(clone.pop(), Some(2));
    assert_eq!(stack.len_shared().to_usize(), 1);
  }

  #[test]
  fn shared_stack_peek() {
    let stack = SharedStack::new(RcStorage::new());
    stack.push_shared(7).unwrap();
    assert_eq!(stack.peek_shared(), Some(7));
    stack.pop_shared();
    assert_eq!(stack.peek_shared(), None);
  }

  #[test]
  fn shared_stack_clear_shared() {
    let stack = SharedStack::new(RcStorage::new());
    stack.push_shared(1).unwrap();
    stack.clear_shared();
    assert!(stack.is_empty());
  }
}
