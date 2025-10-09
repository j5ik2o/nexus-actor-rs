use core::cell::RefCell;

use nexus_utils_core_rs::{
  QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter, RingBuffer, RingQueue, RingStorageBackend,
  DEFAULT_CAPACITY,
};

use crate::sync::RcShared;

/// `Rc`ベースのリングバッファストレージ型エイリアス
///
/// `RcShared`と`RefCell`を使用した参照カウントベースのリングバッファストレージです。
type RcRingStorage<E> = RcShared<RingStorageBackend<RcShared<RefCell<RingBuffer<E>>>>>;

/// `Rc`ベースのリングバッファキュー
///
/// このキューは`no_std`環境で利用可能な、循環バッファを使用したFIFO(First-In-First-Out)キューです。
/// `Rc`と`RefCell`を使用して参照カウントベースの共有所有権を提供します。
///
/// # 特徴
///
/// - **リングバッファ**: 効率的な循環バッファ実装
/// - **動的/静的モード**: 動的拡張または固定容量のいずれかを選択可能
/// - **no_std対応**: 標準ライブラリを必要としません
/// - **クローン可能**: `clone()`で複数のハンドルを作成可能
///
/// # パフォーマンス特性
///
/// - `offer`: O(1)（容量内）、リサイズ時はO(n)
/// - `poll`: O(1)
/// - メモリ使用量: O(capacity)
///
/// # モード
///
/// - **動的モード**: 容量が不足すると自動的に拡張されます（デフォルト）
/// - **静的モード**: 容量制限が厳密に適用され、満杯時に`QueueError::Full`を返します
///
/// # 例
///
/// ```
/// use nexus_utils_embedded_rs::RcRingQueue;
/// use nexus_utils_core_rs::QueueRw;
///
/// // 容量10の動的リングキューを作成
/// let queue = RcRingQueue::new(10);
/// queue.offer(1).unwrap();
/// queue.offer(2).unwrap();
/// assert_eq!(queue.poll().unwrap(), Some(1));
///
/// // 固定容量のリングキューを作成
/// let static_queue = RcRingQueue::new(5).with_dynamic(false);
/// ```
#[derive(Debug, Clone)]
pub struct RcRingQueue<E> {
  inner: RingQueue<RcRingStorage<E>, E>,
}

impl<E> RcRingQueue<E> {
  /// 指定された容量で新しいリングバッファキューを作成します
  ///
  /// デフォルトでは動的拡張モードで作成されます。
  ///
  /// # Arguments
  ///
  /// * `capacity` - キューの初期容量
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcRingQueue;
  ///
  /// let queue: RcRingQueue<String> = RcRingQueue::new(100);
  /// ```
  pub fn new(capacity: usize) -> Self {
    let storage = RcShared::new(RefCell::new(RingBuffer::new(capacity)));
    let backend: RcRingStorage<E> = RcShared::new(RingStorageBackend::new(storage));
    Self {
      inner: RingQueue::new(backend),
    }
  }

  /// 動的拡張モードを設定して自身を返します（ビルダーパターン）
  ///
  /// # Arguments
  ///
  /// * `dynamic` - `true`の場合、容量が不足すると自動的に拡張されます。
  ///               `false`の場合、容量制限が厳密に適用されます。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcRingQueue;
  ///
  /// let queue: RcRingQueue<i32> = RcRingQueue::new(10)
  ///     .with_dynamic(false); // 固定容量モード
  /// ```
  pub fn with_dynamic(mut self, dynamic: bool) -> Self {
    self.inner = self.inner.with_dynamic(dynamic);
    self
  }

  /// キューの動的拡張モードを設定します
  ///
  /// # Arguments
  ///
  /// * `dynamic` - `true`の場合、容量が不足すると自動的に拡張されます
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcRingQueue;
  ///
  /// let queue: RcRingQueue<i32> = RcRingQueue::new(10);
  /// queue.set_dynamic(false); // 固定容量モードに変更
  /// ```
  pub fn set_dynamic(&self, dynamic: bool) {
    self.inner.set_dynamic(dynamic);
  }
}

impl<E> Default for RcRingQueue<E> {
  fn default() -> Self {
    Self::new(DEFAULT_CAPACITY)
  }
}

impl<E> QueueBase<E> for RcRingQueue<E> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E> QueueWriter<E> for RcRingQueue<E> {
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E> QueueReader<E> for RcRingQueue<E> {
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E> QueueRw<E> for RcRingQueue<E> {
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

  #[test]
  fn rc_ring_queue_offer_poll() {
    let queue = RcRingQueue::new(1).with_dynamic(false);
    queue.offer(10).unwrap();
    assert_eq!(queue.poll().unwrap(), Some(10));
    assert_eq!(queue.poll().unwrap(), None);
  }

  #[test]
  fn rc_ring_queue_shared_clone() {
    let queue = RcRingQueue::new(4);
    let cloned = queue.clone();

    queue.offer(1).unwrap();
    cloned.offer(2).unwrap();

    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(cloned.poll().unwrap(), Some(2));
  }

  #[test]
  fn rc_ring_queue_clean_up_resets_state() {
    let queue = RcRingQueue::new(2);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();

    queue.clean_up();
    assert_eq!(queue.len().to_usize(), 0);
    assert!(queue.poll().unwrap().is_none());
  }

  #[test]
  fn rc_ring_queue_dynamic_growth() {
    let queue = RcRingQueue::new(1).with_dynamic(true);
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert_eq!(queue.len().to_usize(), 2);
  }

  #[test]
  fn rc_ring_queue_set_dynamic_switches_mode() {
    let queue = RcRingQueue::new(1);
    queue.set_dynamic(false);
    queue.offer(1).unwrap();
    assert!(matches!(queue.offer(2), Err(QueueError::Full(2))));
  }

  #[test]
  fn rc_ring_queue_trait_interface() {
    let mut queue = RcRingQueue::new(1).with_dynamic(false);
    queue.offer_mut(3).unwrap();
    assert_eq!(queue.poll_mut().unwrap(), Some(3));
  }
}
