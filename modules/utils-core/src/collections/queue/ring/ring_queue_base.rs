use core::fmt::Debug;

pub const DEFAULT_CAPACITY: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueSize {
  Limitless,
  Limited(usize),
}

impl QueueSize {
  pub const fn limitless() -> Self {
    Self::Limitless
  }

  pub const fn limited(value: usize) -> Self {
    Self::Limited(value)
  }

  pub const fn is_limitless(&self) -> bool {
    matches!(self, Self::Limitless)
  }

  /// Convert the queue size into a raw `usize`.
  ///
  /// 無制限（`Limitless`）の場合は `usize::MAX` を返す。テストや統計用途で
  /// 便宜的に使用するためのヘルパとする。
  pub const fn to_usize(self) -> usize {
    match self {
      Self::Limitless => usize::MAX,
      Self::Limited(value) => value,
    }
  }
}

impl Default for QueueSize {
  fn default() -> Self {
    QueueSize::limited(0)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueError<T> {
  Full(T),
  OfferError(T),
  Closed(T),
  Disconnected,
}

pub trait QueueBase<E> {
  fn len(&self) -> QueueSize;
  fn capacity(&self) -> QueueSize;
}

pub trait QueueWriter<E>: QueueBase<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>>;
}

pub trait QueueReader<E>: QueueBase<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>>;
  fn clean_up(&mut self);
}

pub trait SharedQueue<E>: QueueBase<E> {
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>>;
  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>>;
  fn clean_up_shared(&self);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn queue_size_helpers_work_as_expected() {
    let zero = QueueSize::limited(0);
    let limitless = QueueSize::limitless();

    assert!(!zero.is_limitless());
    assert_eq!(zero.to_usize(), 0);

    assert!(limitless.is_limitless());
    assert_eq!(limitless.to_usize(), usize::MAX);

    match limitless {
      QueueSize::Limitless => {}
      _ => panic!("expected limitless variant"),
    }

    match zero {
      QueueSize::Limited(value) => assert_eq!(value, 0),
      _ => panic!("expected limited variant"),
    }
  }
}
