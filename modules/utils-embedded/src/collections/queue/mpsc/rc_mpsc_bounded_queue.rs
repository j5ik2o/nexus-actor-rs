use core::cell::RefCell;

use nexus_utils_core_rs::{
  Element, MpscBuffer, MpscQueue, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter,
  RingBufferBackend,
};

use crate::sync::RcShared;

/// `Rc`ベースの有界MPSC(Multiple Producer, Single Consumer)キュー
///
/// このキューは`no_std`環境で利用可能な、容量制限付きのMPSCキューです。
/// `Rc`と`RefCell`を使用して参照カウントベースの共有所有権を提供します。
///
/// # 特徴
///
/// - **有界**: 指定された容量を超える要素を追加することはできません
/// - **MPSC**: 複数のプロデューサーと単一のコンシューマーをサポート
/// - **no_std対応**: 標準ライブラリを必要としません
/// - **クローン可能**: `clone()`で複数のハンドルを作成可能
///
/// # パフォーマンス特性
///
/// - `offer`: O(1)（容量内）
/// - `poll`: O(1)
/// - メモリ使用量: O(capacity)
///
/// # 例
///
/// ```
/// use nexus_utils_embedded_rs::RcMpscBoundedQueue;
/// use nexus_utils_core_rs::QueueRw;
///
/// let queue = RcMpscBoundedQueue::new(10);
/// queue.offer(42).unwrap();
/// assert_eq!(queue.poll().unwrap(), Some(42));
/// ```
#[derive(Debug, Clone)]
pub struct RcMpscBoundedQueue<E> {
  inner: MpscQueue<RcShared<RingBufferBackend<RefCell<MpscBuffer<E>>>>, E>,
}

impl<E> RcMpscBoundedQueue<E> {
  /// 指定された容量で新しい有界MPSCキューを作成します
  ///
  /// # Arguments
  ///
  /// * `capacity` - キューに格納できる最大要素数
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcMpscBoundedQueue;
  ///
  /// let queue: RcMpscBoundedQueue<u32> = RcMpscBoundedQueue::new(100);
  /// ```
  pub fn new(capacity: usize) -> Self {
    let storage = RcShared::new(RingBufferBackend::new(RefCell::new(MpscBuffer::new(Some(capacity)))));
    Self {
      inner: MpscQueue::new(storage),
    }
  }
}

impl<E: Element> QueueBase<E> for RcMpscBoundedQueue<E> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E: Element> QueueWriter<E> for RcMpscBoundedQueue<E> {
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E: Element> QueueReader<E> for RcMpscBoundedQueue<E> {
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E: Element> QueueRw<E> for RcMpscBoundedQueue<E> {
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  fn clean_up(&self) {
    self.inner.clean_up();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use nexus_utils_core_rs::{QueueBase, QueueRw};

  #[test]
  fn rc_bounded_capacity_limit() {
    let queue: RcMpscBoundedQueue<u32> = RcMpscBoundedQueue::new(1);
    queue.offer(42).unwrap();
    let err = queue.offer(99).unwrap_err();
    assert!(matches!(err, QueueError::Full(99)));
  }

  #[test]
  fn rc_bounded_clean_up_closes_queue() {
    let queue: RcMpscBoundedQueue<u32> = RcMpscBoundedQueue::new(2);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();

    queue.clean_up();
    assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer(3), Err(QueueError::Closed(3))));
  }

  #[test]
  fn rc_bounded_capacity_tracking() {
    let queue: RcMpscBoundedQueue<u32> = RcMpscBoundedQueue::new(2);
    assert_eq!(queue.capacity().to_usize(), 2);
    queue.offer(1).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));
  }
}
