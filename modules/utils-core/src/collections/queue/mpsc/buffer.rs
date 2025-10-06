use alloc::collections::VecDeque;

use crate::collections::{QueueError, QueueSize};

#[derive(Debug, Clone)]
pub struct MpscBuffer<T> {
  buffer: VecDeque<T>,
  capacity: Option<usize>,
  closed: bool,
}

impl<T> MpscBuffer<T> {
  pub fn new(capacity: Option<usize>) -> Self {
    Self {
      buffer: VecDeque::new(),
      capacity,
      closed: false,
    }
  }

  pub fn len(&self) -> QueueSize {
    QueueSize::limited(self.buffer.len())
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
      if self.buffer.len() > limit {
        self.buffer.truncate(limit);
      }
    }
  }

  pub fn offer(&mut self, element: T) -> Result<(), QueueError<T>> {
    if self.closed {
      return Err(QueueError::Closed(element));
    }
    if matches!(self.capacity, Some(limit) if self.buffer.len() >= limit) {
      return Err(QueueError::Full(element));
    }
    self.buffer.push_back(element);
    Ok(())
  }

  pub fn poll(&mut self) -> Result<Option<T>, QueueError<T>> {
    if let Some(item) = self.buffer.pop_front() {
      return Ok(Some(item));
    }
    if self.closed {
      Err(QueueError::Disconnected)
    } else {
      Ok(None)
    }
  }

  pub fn clean_up(&mut self) {
    self.buffer.clear();
    self.closed = true;
  }

  pub fn is_closed(&self) -> bool {
    self.closed
  }

  pub fn mark_closed(&mut self) {
    self.closed = true;
  }
}

impl<T> Default for MpscBuffer<T> {
  fn default() -> Self {
    Self::new(None)
  }
}
