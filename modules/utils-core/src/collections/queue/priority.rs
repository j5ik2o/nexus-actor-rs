use crate::collections::{
  element::Element,
  queue::{QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter},
};
use alloc::vec::Vec;
use core::marker::PhantomData;

/// 優先度キューのレベル数
///
/// デフォルトでは8段階の優先度をサポートします。
/// 0（最低優先度）から7（最高優先度）までの範囲です。
pub const PRIORITY_LEVELS: usize = 8;

/// デフォルトの優先度レベル
///
/// メッセージに優先度が指定されていない場合に使用されます。
/// PRIORITY_LEVELS の中間値（4）がデフォルトとなります。
pub const DEFAULT_PRIORITY: i8 = (PRIORITY_LEVELS / 2) as i8;

/// 優先度を持つメッセージのためのトレイト
///
/// このトレイトを実装することで、メッセージに優先度を付与できます。
/// 優先度は0から7の範囲で指定され、大きい値ほど高い優先度となります。
pub trait PriorityMessage: Element {
  /// メッセージの優先度を取得します
  ///
  /// # Returns
  ///
  /// * `Some(i8)` - 0から7の範囲の優先度。大きい値ほど高優先度
  /// * `None` - 優先度が指定されていない場合、デフォルト優先度が使用されます
  fn get_priority(&self) -> Option<i8>;
}

/// 優先度別キュー
///
/// 複数の優先度レベルを持つキューのファサードです。
/// メッセージを優先度に応じて適切なレベルのキューに振り分けます。
/// 取り出し時は高い優先度のキューから順に処理されます。
///
/// # Type Parameters
///
/// * `Q` - 各レベルで使用するキューの型。`QueueRw<E>`トレイトを実装している必要があります
/// * `E` - キューに格納される要素の型。`PriorityMessage`トレイトを実装している必要があります
#[derive(Debug)]
pub struct PriorityQueue<Q, E>
where
  Q: QueueRw<E>, {
  levels: Vec<Q>,
  _marker: PhantomData<E>,
}

impl<Q, E> PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
{
  /// 新しい優先度キューを作成します
  ///
  /// # Arguments
  ///
  /// * `levels` - 各優先度レベルに対応するキューのベクタ。
  ///              インデックス0が最低優先度、最後のインデックスが最高優先度となります
  ///
  /// # Panics
  ///
  /// `levels`が空の場合にパニックします
  pub fn new(levels: Vec<Q>) -> Self {
    assert!(!levels.is_empty(), "PriorityQueue requires at least one level");
    Self {
      levels,
      _marker: PhantomData,
    }
  }

  /// 各レベルのキューへの不変参照を取得します
  ///
  /// # Returns
  ///
  /// 各優先度レベルのキューのスライス
  pub fn levels(&self) -> &[Q] {
    &self.levels
  }

  /// 各レベルのキューへの可変参照を取得します
  ///
  /// # Returns
  ///
  /// 各優先度レベルのキューの可変スライス
  pub fn levels_mut(&mut self) -> &mut [Q] {
    &mut self.levels
  }

  /// 優先度からレベルインデックスを計算します
  ///
  /// 優先度が範囲外の場合は、0からmax（レベル数-1）の範囲にクランプされます。
  ///
  /// # Arguments
  ///
  /// * `priority` - メッセージの優先度。Noneの場合はデフォルト値（中間レベル）が使用されます
  ///
  /// # Returns
  ///
  /// 0からlevels.len()-1の範囲のインデックス
  fn level_index(&self, priority: Option<i8>) -> usize {
    let levels = self.levels.len();
    let default = (levels / 2) as i8;
    let max = (levels as i32 - 1) as i8;
    let clamped = priority.unwrap_or(default).clamp(0, max);
    clamped as usize
  }

  /// 要素をキューに追加します
  ///
  /// 要素の優先度に基づいて、適切なレベルのキューに要素を追加します。
  ///
  /// # Arguments
  ///
  /// * `element` - 追加する要素
  ///
  /// # Returns
  ///
  /// * `Ok(())` - 正常に追加された場合
  /// * `Err(QueueError)` - キューが満杯などの理由で追加できなかった場合
  pub fn offer(&self, element: E) -> Result<(), QueueError<E>>
  where
    E: PriorityMessage, {
    let idx = self.level_index(element.get_priority());
    self.levels[idx].offer(element)
  }

  /// キューから要素を取り出します
  ///
  /// 最も高い優先度のキューから順に要素を取り出します。
  /// すべてのキューが空の場合は`None`を返します。
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - 要素が取り出せた場合
  /// * `Ok(None)` - すべてのキューが空の場合
  /// * `Err(QueueError)` - エラーが発生した場合
  pub fn poll(&self) -> Result<Option<E>, QueueError<E>>
  where
    E: PriorityMessage, {
    for queue in self.levels.iter().rev() {
      match queue.poll()? {
        Some(item) => return Ok(Some(item)),
        None => continue,
      }
    }
    Ok(None)
  }

  /// すべてのレベルのキューをクリーンアップします
  ///
  /// 各キューの内部状態を整理し、不要なリソースを解放します。
  pub fn clean_up(&self) {
    for queue in &self.levels {
      queue.clean_up();
    }
  }

  /// すべてのレベルのキューの合計長を計算します
  ///
  /// # Returns
  ///
  /// すべてのキューに格納されている要素数の合計。
  /// いずれかのキューが無制限の場合は`QueueSize::Limitless`を返します。
  fn aggregate_len(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.levels {
      match queue.len() {
        QueueSize::Limitless => return QueueSize::limitless(),
        QueueSize::Limited(value) => total += value,
      }
    }
    QueueSize::limited(total)
  }

  /// すべてのレベルのキューの合計容量を計算します
  ///
  /// # Returns
  ///
  /// すべてのキューの容量の合計。
  /// いずれかのキューが無制限の場合は`QueueSize::Limitless`を返します。
  fn aggregate_capacity(&self) -> QueueSize {
    let mut total = 0usize;
    for queue in &self.levels {
      match queue.capacity() {
        QueueSize::Limitless => return QueueSize::limitless(),
        QueueSize::Limited(value) => total += value,
      }
    }
    QueueSize::limited(total)
  }
}

impl<Q, E> Clone for PriorityQueue<Q, E>
where
  Q: QueueRw<E> + Clone,
{
  fn clone(&self) -> Self {
    Self {
      levels: self.levels.clone(),
      _marker: PhantomData,
    }
  }
}

impl<Q, E> QueueBase<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  /// すべてのレベルのキューの合計長を返します
  ///
  /// # Returns
  ///
  /// すべてのキューに格納されている要素数の合計
  fn len(&self) -> QueueSize {
    self.aggregate_len()
  }

  /// すべてのレベルのキューの合計容量を返します
  ///
  /// # Returns
  ///
  /// すべてのキューの容量の合計
  fn capacity(&self) -> QueueSize {
    self.aggregate_capacity()
  }
}

impl<Q, E> QueueWriter<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  /// 要素をキューに追加します（可変参照版）
  ///
  /// 要素の優先度に基づいて、適切なレベルのキューに要素を追加します。
  ///
  /// # Arguments
  ///
  /// * `element` - 追加する要素
  ///
  /// # Returns
  ///
  /// * `Ok(())` - 正常に追加された場合
  /// * `Err(QueueError)` - キューが満杯などの理由で追加できなかった場合
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }
}

impl<Q, E> QueueReader<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  /// キューから要素を取り出します（可変参照版）
  ///
  /// 最も高い優先度のキューから順に要素を取り出します。
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - 要素が取り出せた場合
  /// * `Ok(None)` - すべてのキューが空の場合
  /// * `Err(QueueError)` - エラーが発生した場合
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  /// すべてのレベルのキューをクリーンアップします（可変参照版）
  ///
  /// 各キューの内部状態を整理し、不要なリソースを解放します。
  fn clean_up_mut(&mut self) {
    self.clean_up();
  }
}

impl<Q, E> QueueRw<E> for PriorityQueue<Q, E>
where
  Q: QueueRw<E>,
  E: PriorityMessage,
{
  /// 要素をキューに追加します
  ///
  /// 要素の優先度に基づいて、適切なレベルのキューに要素を追加します。
  ///
  /// # Arguments
  ///
  /// * `element` - 追加する要素
  ///
  /// # Returns
  ///
  /// * `Ok(())` - 正常に追加された場合
  /// * `Err(QueueError)` - キューが満杯などの理由で追加できなかった場合
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }

  /// キューから要素を取り出します
  ///
  /// 最も高い優先度のキューから順に要素を取り出します。
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - 要素が取り出せた場合
  /// * `Ok(None)` - すべてのキューが空の場合
  /// * `Err(QueueError)` - エラーが発生した場合
  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  /// すべてのレベルのキューをクリーンアップします
  ///
  /// 各キューの内部状態を整理し、不要なリソースを解放します。
  fn clean_up(&self) {
    self.clean_up();
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;
  use crate::collections::queue::priority::Vec;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  use super::{PriorityMessage, PriorityQueue};
  use crate::collections::queue::mpsc::{MpscBuffer, MpscHandle, MpscQueue, RingBufferBackend};
  use crate::collections::queue::{QueueBase, QueueReader, QueueRw, QueueWriter};
  use crate::collections::{QueueError, QueueSize};
  use crate::sync::Shared;

  #[derive(Debug, Clone)]
  struct TestQueue(MpscQueue<RcHandle<u32>, u32>);

  #[derive(Debug)]
  struct RcHandle<T>(Rc<RingBufferBackend<RefCell<MpscBuffer<T>>>>);

  impl<T> RcHandle<T> {
    fn new(capacity: Option<usize>) -> Self {
      let buffer = RefCell::new(MpscBuffer::new(capacity));
      let backend = RingBufferBackend::new(buffer);
      Self(Rc::new(backend))
    }
  }

  impl<T> core::ops::Deref for RcHandle<T> {
    type Target = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> Clone for RcHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> Shared<RingBufferBackend<RefCell<MpscBuffer<T>>>> for RcHandle<T> {}

  impl<T> MpscHandle<T> for RcHandle<T> {
    type Backend = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn backend(&self) -> &Self::Backend {
      &self.0
    }
  }

  impl PriorityMessage for u32 {
    fn get_priority(&self) -> Option<i8> {
      Some((*self % 8) as i8)
    }
  }

  impl QueueRw<u32> for TestQueue {
    fn offer(&self, element: u32) -> Result<(), QueueError<u32>> {
      self.0.offer(element)
    }

    fn poll(&self) -> Result<Option<u32>, QueueError<u32>> {
      self.0.poll()
    }

    fn clean_up(&self) {
      self.0.clean_up();
    }
  }

  impl QueueBase<u32> for TestQueue {
    fn len(&self) -> QueueSize {
      self.0.len()
    }

    fn capacity(&self) -> QueueSize {
      self.0.capacity()
    }
  }

  impl QueueWriter<u32> for TestQueue {
    fn offer_mut(&mut self, element: u32) -> Result<(), QueueError<u32>> {
      self.0.offer_mut(element)
    }
  }

  impl QueueReader<u32> for TestQueue {
    fn poll_mut(&mut self) -> Result<Option<u32>, QueueError<u32>> {
      self.0.poll_mut()
    }

    fn clean_up_mut(&mut self) {
      self.0.clean_up_mut();
    }
  }

  impl TestQueue {
    fn bounded(cap: usize) -> Self {
      Self(MpscQueue::new(RcHandle::new(Some(cap))))
    }

    fn unbounded() -> Self {
      Self(MpscQueue::new(RcHandle::new(None)))
    }
  }

  fn sample_levels() -> Vec<TestQueue> {
    (0..super::PRIORITY_LEVELS).map(|_| TestQueue::bounded(4)).collect()
  }

  #[test]
  fn shared_priority_queue_orders_by_priority() {
    let queue = PriorityQueue::new(sample_levels());
    queue.offer(1).unwrap();
    queue.offer(15).unwrap();
    queue.offer(7).unwrap();

    assert_eq!(queue.poll().unwrap(), Some(15));
    assert_eq!(queue.poll().unwrap(), Some(7));
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), None);
  }

  #[test]
  fn shared_priority_queue_len_and_capacity() {
    let queue = PriorityQueue::new(sample_levels());
    let expected = QueueSize::limited(super::PRIORITY_LEVELS * 4);
    assert_eq!(queue.capacity(), expected);
    queue.offer(3).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));
    queue.clean_up();
    assert_eq!(queue.len(), QueueSize::limited(0));
  }

  #[test]
  fn shared_priority_queue_unbounded_capacity() {
    let levels = (0..super::PRIORITY_LEVELS).map(|_| TestQueue::unbounded()).collect();
    let queue = PriorityQueue::new(levels);
    assert!(queue.capacity().is_limitless());
  }
}
