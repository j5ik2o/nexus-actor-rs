use std::sync::Mutex;

use crate::sync::ArcShared;
use nexus_utils_core_rs::{QueueSize, SharedStack, SharedStackHandle, StackBase, StackBuffer, StackError, StackMut};

#[derive(Debug, Clone)]
pub struct Stack<T> {
  inner: SharedStack<ArcShared<Mutex<StackBuffer<T>>>, T>,
}

impl<T> Stack<T> {
  pub fn new() -> Self {
    let storage = ArcShared::new(Mutex::new(StackBuffer::new()));
    Self {
      inner: SharedStack::new(storage),
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

  pub fn push_shared(&self, value: T) -> Result<(), StackError<T>> {
    self.inner.push_shared(value)
  }

  pub fn pop_shared(&self) -> Option<T> {
    self.inner.pop_shared()
  }

  pub fn peek_shared(&self) -> Option<T>
  where
    T: Clone,
  {
    self.inner.peek_shared()
  }

  pub fn clear_shared(&self) {
    self.inner.clear_shared();
  }

  pub fn len_shared(&self) -> QueueSize {
    self.inner.len_shared()
  }

  pub fn capacity_shared(&self) -> QueueSize {
    self.inner.capacity_shared()
  }
}

impl<T> Default for Stack<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> StackBase<T> for Stack<T> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T> StackMut<T> for Stack<T> {
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
    T: Clone,
  {
    self.inner.peek()
  }
}

impl<T> SharedStackHandle<T> for ArcShared<Mutex<StackBuffer<T>>> {
  type Storage = Mutex<StackBuffer<T>>;

  fn storage(&self) -> &Self::Storage {
    &self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn stack_push_pop() {
    let mut stack = Stack::with_capacity(1);
    stack.push(1).unwrap();
    assert!(stack.push(2).is_err());
    assert_eq!(stack.pop(), Some(1));
    assert!(stack.pop().is_none());
  }

  #[test]
  fn stack_shared_access() {
    let stack = Stack::new();
    stack.push_shared(10).unwrap();
    let cloned = stack.clone();
    cloned.push_shared(11).unwrap();

    assert_eq!(stack.len_shared().to_usize(), 2);
    assert_eq!(cloned.pop_shared(), Some(11));
    assert_eq!(stack.pop_shared(), Some(10));
  }

  #[test]
  fn stack_peek_shared() {
    let stack = Stack::new();
    stack.push_shared(5).unwrap();
    assert_eq!(stack.peek_shared(), Some(5));
    stack.pop_shared();
    assert_eq!(stack.peek_shared(), None);
  }
}
