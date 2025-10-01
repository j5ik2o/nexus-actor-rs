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
