use std::sync::Mutex;

use crate::sync::ArcShared;
use nexus_utils_core_rs::{
  QueueSize, Stack, StackBase, StackBuffer, StackError, StackMut, StackStorage, StackStorageBackend,
};

type ArcStackStorage<T> = ArcShared<StackStorageBackend<ArcShared<Mutex<StackBuffer<T>>>>>;

#[derive(Debug, Clone)]
pub struct ArcStack<T> {
  inner: Stack<ArcStackStorage<T>, T>,
}

impl<T> ArcStack<T> {
  pub fn new() -> Self {
    let storage = ArcShared::new(Mutex::new(StackBuffer::new()));
    let backend: ArcStackStorage<T> = ArcShared::new(StackStorageBackend::new(storage));
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

  pub fn push(&self, value: T) -> Result<(), StackError<T>> {
    self.inner.push(value)
  }

  pub fn pop(&self) -> Option<T> {
    self.inner.pop()
  }

  pub fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.inner.peek()
  }

  pub fn clear(&self) {
    self.inner.clear();
  }

  pub fn len(&self) -> QueueSize {
    self.inner.len()
  }

  pub fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T> Default for ArcStack<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> StackBase<T> for ArcStack<T> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T> StackMut<T> for ArcStack<T> {
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

impl<T> StackStorage<T> for ArcShared<Mutex<StackBuffer<T>>> {
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
    let guard = self.lock().expect("mutex poisoned");
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
    let mut guard = self.lock().expect("mutex poisoned");
    f(&mut guard)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn stack_push_pop() {
    let stack = ArcStack::with_capacity(1);
    stack.push(1).unwrap();
    assert!(stack.push(2).is_err());
    assert_eq!(stack.pop(), Some(1));
    assert!(stack.pop().is_none());
  }

  #[test]
  fn stack_handle_access() {
    let stack = ArcStack::new();
    stack.push(10).unwrap();
    let cloned = stack.clone();
    cloned.push(11).unwrap();

    assert_eq!(stack.len().to_usize(), 2);
    assert_eq!(cloned.pop(), Some(11));
    assert_eq!(stack.pop(), Some(10));
  }

  #[test]
  fn stack_peek_ref() {
    let stack = ArcStack::new();
    stack.push(5).unwrap();
    assert_eq!(stack.peek(), Some(5));
    stack.pop();
    assert_eq!(stack.peek(), None);
  }
}
