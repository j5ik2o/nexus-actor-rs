use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt::{self, Debug};
use core::ops::Add;

use super::Element;
#[derive(Debug)]
pub enum QueueError<E> {
  OfferError(E),
  PoolError,
  PeekError,
  ContainsError,
  InterruptedError,
  TimeoutError,
}

impl<E: Debug> fmt::Display for QueueError<E> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      QueueError::OfferError(e) => write!(f, "Failed to offer an element: {e:?}"),
      QueueError::PoolError => write!(f, "Failed to poll an element"),
      QueueError::PeekError => write!(f, "Failed to peek an element"),
      QueueError::ContainsError => write!(f, "Failed to check whether an element exists"),
      QueueError::InterruptedError => write!(f, "Failed to interrupt"),
      QueueError::TimeoutError => write!(f, "Failed to timeout"),
    }
  }
}

#[cfg(feature = "std")]
impl<E: Debug> std::error::Error for QueueError<E> {}

#[derive(Debug, Clone, Copy)]
pub enum QueueSize {
  Limitless,
  Limited(usize),
}

impl QueueSize {
  pub fn is_limitless(&self) -> bool {
    matches!(self, QueueSize::Limitless)
  }

  pub fn to_option(&self) -> Option<usize> {
    match self {
      QueueSize::Limitless => None,
      QueueSize::Limited(size) => Some(*size),
    }
  }

  pub fn to_usize(&self) -> usize {
    match self {
      QueueSize::Limitless => usize::MAX,
      QueueSize::Limited(size) => *size,
    }
  }
}

impl Add for QueueSize {
  type Output = QueueSize;

  fn add(self, other: QueueSize) -> QueueSize {
    match (self, other) {
      (QueueSize::Limitless, _) | (_, QueueSize::Limitless) => QueueSize::Limitless,
      (QueueSize::Limited(lhs), QueueSize::Limited(rhs)) => QueueSize::Limited(lhs + rhs),
    }
  }
}

impl PartialEq for QueueSize {
  fn eq(&self, other: &Self) -> bool {
    matches!((self, other), (QueueSize::Limitless, QueueSize::Limitless))
      || matches!((self, other), (QueueSize::Limited(lhs), QueueSize::Limited(rhs)) if lhs == rhs)
  }
}

impl PartialOrd for QueueSize {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    match (self, other) {
      (QueueSize::Limitless, QueueSize::Limitless) => Some(Ordering::Equal),
      (QueueSize::Limitless, _) => Some(Ordering::Greater),
      (_, QueueSize::Limitless) => Some(Ordering::Less),
      (QueueSize::Limited(lhs), QueueSize::Limited(rhs)) => lhs.partial_cmp(rhs),
    }
  }
}

pub trait QueueSupport {}

pub trait QueueBase<E: Element>: Debug + Send + Sync {
  fn len(&self) -> QueueSize;
  fn capacity(&self) -> QueueSize;

  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }

  fn is_full(&self) -> bool {
    self.capacity() == self.len()
  }

  fn non_empty(&self) -> bool {
    !self.is_empty()
  }

  fn non_full(&self) -> bool {
    !self.is_full()
  }
}

pub trait QueueWriter<E: Element>: QueueBase<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>>;

  fn offer_all<I>(&mut self, elements: I) -> Result<(), QueueError<E>>
  where
    I: IntoIterator<Item = E>, {
    for element in elements {
      self.offer(element)?;
    }
    Ok(())
  }
}

pub trait QueueReader<E: Element>: QueueBase<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>>;
  fn clean_up(&mut self);
}

#[derive(Debug, Clone)]
pub struct RingBuffer<E> {
  buffer: Vec<Option<E>>,
  head: usize,
  tail: usize,
  len: usize,
  dynamic: bool,
}

impl<E> RingBuffer<E> {
  pub fn new(capacity: usize) -> Self {
    assert!(capacity > 0, "Capacity must be greater than zero");
    let mut buffer = Vec::with_capacity(capacity);
    buffer.resize_with(capacity, || None);
    Self {
      buffer,
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

  fn resize(&mut self) {
    let old_capacity = self.buffer.len();
    let new_capacity = old_capacity * 2 + 1;
    let mut new_buffer = Vec::with_capacity(new_capacity);
    new_buffer.resize_with(new_capacity, || None);

    for idx in 0..self.len {
      let source = (self.head + idx) % old_capacity;
      new_buffer[idx] = self.buffer[source].take();
    }

    self.buffer = new_buffer;
    self.head = 0;
    self.tail = self.len;
  }
}

impl<E: Element> QueueBase<E> for RingBuffer<E> {
  fn len(&self) -> QueueSize {
    QueueSize::Limited(self.len)
  }

  fn capacity(&self) -> QueueSize {
    QueueSize::Limited(self.buffer.len())
  }
}

impl<E: Element> QueueWriter<E> for RingBuffer<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    let capacity = self.buffer.len();
    let max_len = capacity.saturating_sub(1);
    if self.len == max_len {
      if self.dynamic {
        self.resize();
      } else {
        return Err(QueueError::OfferError(element));
      }
    }
    let capacity = self.buffer.len();
    self.buffer[self.tail] = Some(element);
    self.tail = (self.tail + 1) % capacity;
    self.len += 1;
    Ok(())
  }
}

impl<E: Element> QueueReader<E> for RingBuffer<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    if self.len == 0 {
      return Ok(None);
    }
    let capacity = self.buffer.len();
    let element = self.buffer[self.head].take();
    self.head = (self.head + 1) % capacity;
    self.len -= 1;
    Ok(element)
  }

  fn clean_up(&mut self) {
    self.buffer.iter_mut().for_each(|slot| *slot = None);
    self.head = 0;
    self.tail = 0;
    self.len = 0;
  }
}

impl<E: Element> QueueSupport for RingBuffer<E> {}
