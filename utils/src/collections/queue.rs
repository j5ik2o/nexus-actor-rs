use std::cmp::Ordering;
use std::fmt::Debug;
use std::ops::Add;

use async_trait::async_trait;
use thiserror::Error;

mod mpsc_bounded_channel_queue;
mod mpsc_unbounded_channel_queue;
mod priority_queue;
mod ring_queue;

pub use self::{mpsc_bounded_channel_queue::*, mpsc_unbounded_channel_queue::*, priority_queue::*, ring_queue::*};

use crate::collections::element::Element;

/// An error that occurs when a queue operation fails.<br/>
/// キューの操作に失敗した場合に発生するエラー。
#[derive(Error, Debug, PartialEq)]
pub enum QueueError<E> {
  #[error("Failed to offer an element: {0:?}")]
  OfferError(E),
  #[error("Failed to pool an element")]
  PoolError,
  #[error("Failed to peek an element")]
  PeekError,
  #[error("Failed to contains an element")]
  ContainsError,
  #[error("Failed to interrupt")]
  InterruptedError,
  #[error("Failed to timeout")]
  TimeoutError,
}

/// The size of the queue.<br/>
/// キューのサイズ。
#[derive(Debug, Clone)]
pub enum QueueSize {
  /// The queue has no capacity limit.<br/>
  /// キューに容量制限がない。
  Limitless,
  /// The queue has a capacity limit.<br/>
  /// キューに容量制限がある。
  Limited(usize),
}

impl QueueSize {
  /// Returns whether the queue has no capacity limit.<br/>
  /// キューに容量制限がないかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `false` - If the queue has a capacity limit. / キューに容量制限がある場合。
  pub fn is_limitless(&self) -> bool {
    matches!(self, QueueSize::Limitless)
  }

  /// Converts to an option type.<br/>
  /// オプション型に変換します。
  ///
  /// # Return Value / 戻り値
  /// - `None` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `Some(num)` - If the queue has a capacity limit. / キューに容量制限がある場合。
  pub fn to_option(&self) -> Option<usize> {
    match self {
      QueueSize::Limitless => None,
      QueueSize::Limited(c) => Some(*c),
    }
  }

  /// Converts to a usize type.<br/>
  /// usize型に変換します。
  ///
  /// # Return Value / 戻り値
  /// - `usize::MAX` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `num` - If the queue has a capacity limit. / キューに容量制限がある場合。
  pub fn to_usize(&self) -> usize {
    match self {
      QueueSize::Limitless => usize::MAX,
      QueueSize::Limited(c) => *c,
    }
  }
}

impl Add for QueueSize {
  type Output = QueueSize;

  fn add(self, other: QueueSize) -> QueueSize {
    match (self, other) {
      (QueueSize::Limitless, _) | (_, QueueSize::Limitless) => QueueSize::Limitless,
      (QueueSize::Limited(a), QueueSize::Limited(b)) => QueueSize::Limited(a + b),
    }
  }
}

impl PartialEq<Self> for QueueSize {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (QueueSize::Limitless, QueueSize::Limitless) => true,
      (QueueSize::Limited(l), QueueSize::Limited(r)) => l == r,
      _ => false,
    }
  }
}

impl PartialOrd<Self> for QueueSize {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    match (self, other) {
      (QueueSize::Limitless, QueueSize::Limitless) => Some(Ordering::Equal),
      (QueueSize::Limitless, _) => Some(Ordering::Greater),
      (_, QueueSize::Limitless) => Some(Ordering::Less),
      (QueueSize::Limited(l), QueueSize::Limited(r)) => l.partial_cmp(r),
    }
  }
}

/// A trait that defines the behavior of a queue.<br/>
/// キューの振る舞いを定義するトレイト。
#[async_trait]
pub trait QueueBase<E: Element>: Debug + Send + Sync {
  /// Returns whether this queue is empty.<br/>
  /// このキューが空かどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue is empty. / キューが空の場合。
  /// - `false` - If the queue is not empty. / キューが空でない場合。
  async fn is_empty(&self) -> bool {
    self.len().await == QueueSize::Limited(0)
  }

  /// Returns whether this queue is non-empty.<br/>
  /// このキューが空でないかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue is not empty. / キューが空でない場合。
  /// - `false` - If the queue is empty. / キューが空の場合。
  async fn non_empty(&self) -> bool {
    !self.is_empty().await
  }

  /// Returns whether the queue size has reached its capacity.<br/>
  /// このキューのサイズが容量まで到達したかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue size has reached its capacity. / キューのサイズが容量まで到達した場合。
  /// - `false` - If the queue size has not reached its capacity. / キューのサイズが容量まで到達してない場合。
  async fn is_full(&self) -> bool {
    self.capacity().await == self.len().await
  }

  /// Returns whether the queue size has not reached its capacity.<br/>
  /// このキューのサイズが容量まで到達してないかどうかを返します。
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the queue size has not reached its capacity. / キューのサイズが容量まで到達してない場合。
  /// - `false` - If the queue size has reached its capacity. / キューのサイズが容量まで到達した場合。
  async fn non_full(&self) -> bool {
    !self.is_full().await
  }

  /// Returns the length of this queue.<br/>
  /// このキューの長さを返します。
  ///
  /// # Return Value / 戻り値
  /// - `QueueSize::Limitless` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `QueueSize::Limited(num)` - If the queue has a capacity limit. / キューに容量制限がある場合。
  async fn len(&self) -> QueueSize;

  /// Returns the capacity of this queue.<br/>
  /// このキューの最大容量を返します。
  ///
  /// # Return Value / 戻り値
  /// - `QueueSize::Limitless` - If the queue has no capacity limit. / キューに容量制限がない場合。
  /// - `QueueSize::Limited(num)` - If the queue has a capacity limit. / キューに容量制限がある場合。
  async fn capacity(&self) -> QueueSize;
}

#[async_trait]
pub trait QueueWriteFactory<E: Element>: QueueBase<E> {
  type Writer: QueueWriter<E>;
  fn writer(&self) -> Self::Writer;
}

#[async_trait::async_trait]
pub trait QueueWriter<E: Element>: QueueBase<E> {
  /// The specified element will be inserted into this queue,
  /// if the queue can be executed immediately without violating the capacity limit.<br/>
  /// 容量制限に違反せずにすぐ実行できる場合は、指定された要素をこのキューに挿入します。
  ///
  /// # Arguments / 引数
  /// - `element` - The element to be inserted. / 挿入する要素。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(())` - If the element is inserted successfully. / 要素が正常に挿入された場合。
  /// - `Err(QueueError::OfferError(element))` - If the element cannot be inserted. / 要素を挿入できなかった場合。
  async fn offer(&mut self, element: E) -> Result<(), QueueError<E>>;

  /// The specified elements will be inserted into this queue,
  /// if the queue can be executed immediately without violating the capacity limit.<br/>
  /// 容量制限に違反せずにすぐ実行できる場合は、指定された複数の要素をこのキューに挿入します。
  ///
  /// # Arguments / 引数
  /// - `elements` - The elements to be inserted. / 挿入する要素。
  ///
  /// # Return Value / 戻り値
  /// - `Ok(())` - If the elements are inserted successfully. / 要素が正常に挿入された場合。
  /// - `Err(QueueError::OfferError(element))` - If the elements cannot be inserted. / 要素を挿入できなかった場合。
  async fn offer_all(&mut self, elements: Vec<E>) -> Result<(), QueueError<E>> {
    for e in elements {
      self.offer(e).await?;
    }
    Ok(())
  }
}

#[async_trait::async_trait]
pub trait QueueReadFactory<E: Element>: QueueBase<E> {
  type Reader: QueueReader<E>;
  fn reader(&self) -> Self::Reader;
}

#[async_trait]
pub trait QueueReader<E: Element>: QueueBase<E> {
  /// Retrieves and deletes the head of the queue. Returns None if the queue is empty.
  ///
  /// # Return Value
  /// - `Ok(Some(element))` - If the element is retrieved successfully.
  /// - `Ok(None)` - If the queue is empty.
  async fn poll(&mut self) -> Result<Option<E>, QueueError<E>>;

  async fn poll_many(&mut self, n: usize) -> Result<Vec<E>, QueueError<E>> {
    let mut elements = Vec::with_capacity(n);
    for _ in 0..n {
      let result = self.poll().await?;
      if let Some(element) = result {
        elements.push(element);
      }
    }
    Ok(elements)
  }

  async fn clean_up(&mut self);
}

/// A trait that defines the behavior of a queue that can be peeked.
#[async_trait]
pub trait HasPeekBehavior<E: Element>: QueueReader<E> {
  /// Gets the head of the queue, but does not delete it. Returns None if the queue is empty.
  ///
  /// # Return Value
  /// - `Ok(Some(element))` - If the element is retrieved successfully.
  /// - `Ok(None)` - If the queue is empty.
  async fn peek(&self) -> Result<Option<E>, QueueError<E>>;
}

/// A trait that defines the behavior of a queue that can be checked for contains.
#[async_trait]
pub trait HasContainsBehavior<E: Element>: QueueReader<E> {
  /// Returns whether the specified element is contained in this queue.
  ///
  /// # Arguments
  /// - `element` - The element to be checked.
  ///
  /// # Return Value
  /// - `true` - If the element is contained in this queue.
  /// - `false` - If the element is not contained in this queue.
  async fn contains(&self, element: &E) -> bool;
}

/// A trait that defines the behavior of a blocking queue.
#[async_trait]
pub trait BlockingQueueBase<E: Element>: QueueBase<E> + Send {
  /// Returns the number of elements that can be inserted into this queue without blocking.
  ///
  /// # Return Value
  /// - `QueueSize::Limitless` - If the queue has no capacity limit.
  /// - `QueueSize::Limited(num)` - If the queue has a capacity limit.
  async fn remaining_capacity(&self) -> QueueSize;

  /// Returns whether the operation of this queue has been interrupted.
  ///
  /// # Return Value / 戻り値
  /// - `true` - If the operation is interrupted.
  /// - `false` - If the operation is not interrupted.
  async fn is_interrupted(&self) -> bool;
}

#[async_trait]
pub trait BlockingQueueWriter<E: Element>: BlockingQueueBase<E> + QueueWriter<E> {
  /// Inserts the specified element into this queue. If necessary, waits until space is available.
  ///
  /// # Arguments
  /// - `element` - The element to be inserted.
  ///
  /// # Return Value
  /// - `Ok(())` - If the element is inserted successfully.
  /// - `Err(QueueError::OfferError(element))` - If the element cannot be inserted.
  /// - `Err(QueueError::InterruptedError)` - If the operation is interrupted.
  async fn put(&mut self, element: E) -> Result<(), QueueError<E>>;

  /// Interrupts the operation of this queue.
  async fn interrupt(&mut self);
}

#[async_trait]
pub trait BlockingQueueReader<E: Element>: BlockingQueueBase<E> {
  /// Retrieve the head of this queue and delete it. If necessary, wait until an element becomes available.
  ///
  /// # Return Value
  /// - `Ok(Some(element))` - If the element is retrieved successfully.
  /// - `Ok(None)` - If the queue is empty.
  /// - `Err(QueueError::InterruptedError)` - If the operation is interrupted.
  async fn take(&mut self) -> Result<Option<E>, QueueError<E>>;
}
