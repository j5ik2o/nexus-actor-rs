use core::cell::RefCell;

use nexus_utils_core_rs::{
  QueueSize, Stack, StackBase, StackBuffer, StackError, StackMut, StackStorage, StackStorageBackend,
};

use crate::sync::RcShared;

#[derive(Debug, Clone)]
pub struct RcStack<T> {
  inner: Stack<RcShared<StackStorageBackend<RcShared<RefCell<StackBuffer<T>>>>>, T>,
}

impl<T> RcStack<T> {
  pub fn new() -> Self {
    let storage = RcShared::new(RefCell::new(StackBuffer::new()));
    let backend = RcShared::new(StackStorageBackend::new(storage));
    Self {
      inner: Stack::new(backend),
    }
  }

  pub fn with_capacity(capacity: usize) -> Self {
    let stack = Self::new();
    stack.set_capacity(Some(capacity));
    stack
  }

  pub fn set_capacity(&self, capacity: Option<usize>) {
    self.inner.set_capacity(capacity);
  }

  pub fn push_ref(&self, value: T) -> Result<(), StackError<T>> {
    self.inner.push_ref(value)
  }

  pub fn pop_ref(&self) -> Option<T> {
    self.inner.pop_ref()
  }

  pub fn peek_ref(&self) -> Option<T>
  where
    T: Clone, {
    self.inner.peek_ref()
  }

  pub fn clear_ref(&self) {
    self.inner.clear_ref();
  }

  pub fn len_ref(&self) -> QueueSize {
    self.inner.len_ref()
  }

  pub fn capacity_ref(&self) -> QueueSize {
    self.inner.capacity_ref()
  }
}

impl<T> Default for RcStack<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> StackBase<T> for RcStack<T> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T> StackMut<T> for RcStack<T> {
  fn push(&mut self, value: T) -> Result<(), StackError<T>> {
    self.inner.push(value)
  }

  fn pop(&mut self) -> Option<T> {
    self.inner.pop()
  }

  fn clear(&mut self) {
    self.inner.clear();
  }

  fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.inner.peek()
  }
}

impl<T> StackStorage<T> for RcShared<RefCell<StackBuffer<T>>> {
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
    f(&self.borrow())
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
    f(&mut self.borrow_mut())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rc_stack_push_pop() {
    let mut stack = RcStack::with_capacity(1);
    stack.push(1).unwrap();
    assert!(stack.push(2).is_err());
    assert_eq!(stack.pop(), Some(1));
    assert!(stack.pop().is_none());
  }

  #[test]
  fn rc_stack_handle_operations() {
    let stack = RcStack::new();
    stack.push_ref(10).unwrap();
    let cloned = stack.clone();
    cloned.push_ref(11).unwrap();

    assert_eq!(stack.len_ref().to_usize(), 2);
    assert_eq!(cloned.pop_ref(), Some(11));
    assert_eq!(stack.pop_ref(), Some(10));
  }

  #[test]
  fn rc_stack_peek_ref() {
    let stack = RcStack::new();
    stack.push_ref(5).unwrap();
    assert_eq!(stack.peek_ref(), Some(5));
    stack.pop_ref();
    assert_eq!(stack.peek_ref(), None);
  }
}
