use super::traits::{StackBackend, StackBase, StackHandle, StackMut};
use crate::collections::QueueSize;

use super::StackError;

/// Stack facade that delegates to a [`StackBackend`].
#[derive(Debug)]
pub struct Stack<H, T>
where
  H: StackHandle<T>,
{
  backend: H,
  _marker: core::marker::PhantomData<T>,
}

impl<H, T> Stack<H, T>
where
  H: StackHandle<T>,
{
  pub fn new(backend: H) -> Self {
    Self {
      backend,
      _marker: core::marker::PhantomData,
    }
  }

  pub fn backend(&self) -> &H {
    &self.backend
  }

  pub fn into_backend(self) -> H {
    self.backend
  }

  pub fn set_capacity(&self, capacity: Option<usize>) {
    self.backend.backend().set_capacity(capacity);
  }

  pub fn push(&self, value: T) -> Result<(), StackError<T>> {
    self.backend.backend().push(value)
  }

  pub fn pop(&self) -> Option<T> {
    self.backend.backend().pop()
  }

  pub fn peek(&self) -> Option<T>
  where
    T: Clone,
  {
    self.backend.backend().peek()
  }

  pub fn clear(&self) {
    self.backend.backend().clear();
  }

  pub fn len(&self) -> QueueSize {
    self.backend.backend().len()
  }

  pub fn capacity(&self) -> QueueSize {
    self.backend.backend().capacity()
  }
}

impl<H, T> Clone for Stack<H, T>
where
  H: StackHandle<T>,
{
  fn clone(&self) -> Self {
    Self {
      backend: self.backend.clone(),
      _marker: core::marker::PhantomData,
    }
  }
}

impl<H, T> StackBase<T> for Stack<H, T>
where
  H: StackHandle<T>,
{
  fn len(&self) -> QueueSize {
    self.backend.backend().len()
  }

  fn capacity(&self) -> QueueSize {
    self.backend.backend().capacity()
  }
}

impl<H, T> StackMut<T> for Stack<H, T>
where
  H: StackHandle<T>,
{
  fn push(&mut self, value: T) -> Result<(), StackError<T>> {
    self.backend.backend().push(value)
  }

  fn pop(&mut self) -> Option<T> {
    self.backend.backend().pop()
  }

  fn clear(&mut self) {
    self.backend.backend().clear();
  }

  fn peek(&self) -> Option<T>
  where
    T: Clone,
  {
    self.backend.backend().peek()
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  use super::*;
  use crate::collections::stack::buffer::StackBuffer;
  use crate::collections::stack::traits::{StackHandle, StackStorage, StackStorageBackend};
  use crate::sync::Shared;

  struct RcStorageHandle<T>(Rc<RefCell<StackBuffer<T>>>);

  impl<T> Clone for RcStorageHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> core::ops::Deref for RcStorageHandle<T> {
    type Target = RefCell<StackBuffer<T>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> StackStorage<T> for RcStorageHandle<T> {
    fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
      f(&self.borrow())
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
      f(&mut self.borrow_mut())
    }
  }

  impl<T> Shared<RefCell<StackBuffer<T>>> for RcStorageHandle<T> {}

  struct RcBackendHandle<T>(Rc<StackStorageBackend<RcStorageHandle<T>>>);

  impl<T> Clone for RcBackendHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> core::ops::Deref for RcBackendHandle<T> {
    type Target = StackStorageBackend<RcStorageHandle<T>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> Shared<StackStorageBackend<RcStorageHandle<T>>> for RcBackendHandle<T> {}

  impl<T> StackHandle<T> for RcBackendHandle<T> {
    type Backend = StackStorageBackend<RcStorageHandle<T>>;

    fn backend(&self) -> &Self::Backend {
      &self.0
    }
  }

  #[test]
  fn stack_push_pop_via_handle() {
    let storage = RcStorageHandle(Rc::new(RefCell::new(StackBuffer::new())));
    let backend = RcBackendHandle(Rc::new(StackStorageBackend::new(storage)));
    let stack = Stack::new(backend.clone());

    stack.set_capacity(Some(2));
    stack.push(1).unwrap();
    stack.push(2).unwrap();
    assert!(stack.push(3).is_err());
    assert_eq!(stack.pop(), Some(2));
    assert_eq!(backend.backend().len().to_usize(), 1);
  }

  #[test]
  fn stack_peek_via_handle() {
    let storage = RcStorageHandle(Rc::new(RefCell::new(StackBuffer::new())));
    let backend = RcBackendHandle(Rc::new(StackStorageBackend::new(storage)));
    let stack = Stack::new(backend);

    stack.push(7).unwrap();
    assert_eq!(stack.peek(), Some(7));
    stack.pop();
    assert_eq!(stack.peek(), None);
  }

  #[test]
  fn stack_clear_via_handle() {
    let storage = RcStorageHandle(Rc::new(RefCell::new(StackBuffer::new())));
    let backend = RcBackendHandle(Rc::new(StackStorageBackend::new(storage)));
    let stack = Stack::new(backend);

    stack.push(1).unwrap();
    stack.clear();
    assert!(stack.is_empty());
  }
}
