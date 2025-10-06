use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::mem::MaybeUninit;

use crate::collections::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

pub const DEFAULT_CAPACITY: usize = 32;

#[derive(Debug)]
pub struct RingBuffer<T> {
  buf: Box<[MaybeUninit<T>]>,
  head: usize,
  tail: usize,
  len: usize,
  dynamic: bool,
}

impl<T> RingBuffer<T> {
  pub fn new(capacity: usize) -> Self {
    assert!(capacity > 0, "capacity must be > 0");
    let buf = Self::alloc_buffer(capacity);
    Self {
      buf,
      head: 0,
      tail: 0,
      len: 0,
      dynamic: true,
    }
  }

  pub fn with_dynamic(mut self, dynamic: bool) -> Self {
    self.dynamic = dynamic;
    self
  }

  pub fn set_dynamic(&mut self, dynamic: bool) {
    self.dynamic = dynamic;
  }

  fn alloc_buffer(capacity: usize) -> Box<[MaybeUninit<T>]> {
    let mut vec = Vec::with_capacity(capacity);
    vec.resize_with(capacity, MaybeUninit::uninit);
    vec.into_boxed_slice()
  }

  fn grow(&mut self) {
    let new_cap = self.buf.len().saturating_mul(2).max(1);
    let mut items = Vec::with_capacity(self.len);
    while let Ok(Some(item)) = self.poll_mut() {
      items.push(item);
    }

    self.buf = Self::alloc_buffer(new_cap);
    self.head = 0;
    self.tail = 0;
    self.len = 0;

    for item in items {
      // reinsert without triggering another grow
      if let Err(QueueError::Full(_)) = self.offer_mut(item) {
        panic!("ring buffer grow failed to reserve capacity");
      }
    }
  }
}

impl<T> QueueBase<T> for RingBuffer<T> {
  fn len(&self) -> QueueSize {
    QueueSize::limited(self.len)
  }

  fn capacity(&self) -> QueueSize {
    if self.dynamic {
      QueueSize::limitless()
    } else {
      QueueSize::limited(self.buf.len())
    }
  }
}

impl<T> QueueWriter<T> for RingBuffer<T> {
  fn offer_mut(&mut self, item: T) -> Result<(), QueueError<T>> {
    if self.len == self.buf.len() {
      if self.dynamic {
        self.grow();
      } else {
        return Err(QueueError::Full(item));
      }
    }

    self.buf[self.tail] = MaybeUninit::new(item);
    self.tail = (self.tail + 1) % self.buf.len();
    self.len += 1;
    Ok(())
  }
}

impl<T> QueueReader<T> for RingBuffer<T> {
  fn poll_mut(&mut self) -> Result<Option<T>, QueueError<T>> {
    if self.len == 0 {
      return Ok(None);
    }

    let value = unsafe { self.buf[self.head].assume_init_read() };
    self.head = (self.head + 1) % self.buf.len();
    self.len -= 1;
    Ok(Some(value))
  }

  fn clean_up_mut(&mut self) {
    while self.len > 0 {
      let _ = self.poll_mut();
    }
  }
}

impl<T> Default for RingBuffer<T> {
  fn default() -> Self {
    Self::new(DEFAULT_CAPACITY)
  }
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;

  #[test]
  fn ring_buffer_offer_poll() {
    let mut buffer = RingBuffer::new(2).with_dynamic(false);
    buffer.offer_mut(1).unwrap();
    buffer.offer_mut(2).unwrap();
    assert_eq!(buffer.offer_mut(3), Err(QueueError::Full(3)));

    assert_eq!(buffer.poll_mut().unwrap(), Some(1));
    assert_eq!(buffer.poll_mut().unwrap(), Some(2));
    assert_eq!(buffer.poll_mut().unwrap(), None);
  }

  #[test]
  fn ring_buffer_grows_when_dynamic() {
    let mut buffer = RingBuffer::new(1);
    buffer.offer_mut(1).unwrap();
    buffer.offer_mut(2).unwrap();
    assert_eq!(buffer.len().to_usize(), 2);
    assert!(buffer.capacity().is_limitless());
  }
}
