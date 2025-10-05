use alloc::vec::Vec;

use crate::collections::{QueueError, QueueSize};

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
