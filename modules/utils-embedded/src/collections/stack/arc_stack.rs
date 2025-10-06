use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use nexus_utils_core_rs::{
  QueueSize, Stack, StackBase, StackBuffer, StackError, StackMut, StackStorage, StackStorageBackend, StateCell,
};

use crate::sync::{ArcShared, ArcStateCell};

#[derive(Debug)]
pub struct ArcStack<T, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: Stack<ArcShared<StackStorageBackend<ArcShared<ArcStateCell<StackBuffer<T>, RM>>>>, T>,
}

pub type ArcLocalStack<T> = ArcStack<T, NoopRawMutex>;
pub type ArcCsStack<T> = ArcStack<T, CriticalSectionRawMutex>;

impl<T, RM> ArcStack<T, RM>
where
  RM: RawMutex,
{
  pub fn new() -> Self {
    let storage = ArcShared::new(ArcStateCell::new(StackBuffer::new()));
    let backend = ArcShared::new(StackStorageBackend::new(storage));
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

impl<T, RM> Default for ArcStack<T, RM>
where
  RM: RawMutex,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<T, RM> Clone for ArcStack<T, RM>
where
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

impl<T, RM> StackBase<T> for ArcStack<T, RM>
where
  RM: RawMutex,
{
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T, RM> StackMut<T> for ArcStack<T, RM>
where
  RM: RawMutex,
{
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

impl<T, RM> StackStorage<T> for ArcStateCell<StackBuffer<T>, RM>
where
  RM: RawMutex,
{
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
    let guard = self.borrow();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
    let mut guard = self.borrow_mut();
    f(&mut guard)
  }
}

impl<T, RM> StackStorage<T> for ArcShared<ArcStateCell<StackBuffer<T>, RM>>
where
  RM: RawMutex,
{
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
    (**self).with_read(f)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
    (**self).with_write(f)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tests::init_arc_critical_section;

  fn prepare() {
    init_arc_critical_section();
  }

  #[test]
  fn arc_stack_push_pop() {
    prepare();
    let mut stack: ArcStack<u32> = ArcLocalStack::with_capacity(1);
    stack.push(1).unwrap();
    assert!(stack.push(2).is_err());
    assert_eq!(stack.pop(), Some(1));
    assert!(stack.pop().is_none());
  }

  #[test]
  fn arc_stack_handle_operations() {
    prepare();
    let stack: ArcStack<u32> = ArcLocalStack::new();
    stack.push_ref(10).unwrap();
    let cloned = stack.clone();
    cloned.push_ref(11).unwrap();

    assert_eq!(stack.len_ref().to_usize(), 2);
    assert_eq!(cloned.pop_ref(), Some(11));
    assert_eq!(stack.pop_ref(), Some(10));
  }

  #[test]
  fn arc_stack_peek_ref() {
    prepare();
    let stack: ArcStack<u32> = ArcLocalStack::new();
    stack.push_ref(5).unwrap();
    assert_eq!(stack.peek_ref(), Some(5));
    stack.pop_ref();
    assert_eq!(stack.peek_ref(), None);
  }
}
