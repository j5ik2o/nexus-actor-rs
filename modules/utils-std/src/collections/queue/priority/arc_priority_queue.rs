use crate::ArcRingQueue;
use nexus_utils_core_rs::{
  PriorityMessage, PriorityQueue, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter, PRIORITY_LEVELS,
};

/// 優先度付きキュー
///
/// メッセージの優先度に基づいて要素を取り出す優先度キューです。
/// 内部的に複数のリングキューレベルを使用し、高優先度のメッセージを先に処理します。
#[derive(Debug, Clone)]
pub struct ArcPriorityQueue<E> {
  inner: PriorityQueue<ArcRingQueue<E>, E>,
}

impl<E> ArcPriorityQueue<E> {
  /// 優先度レベルごとに指定された容量で新しい優先度キューを作成します
  ///
  /// # Arguments
  ///
  /// * `capacity_per_level` - 各優先度レベルのキュー容量
  ///
  /// # Returns
  ///
  /// 新しい優先度キューインスタンス
  pub fn new(capacity_per_level: usize) -> Self {
    let levels = (0..PRIORITY_LEVELS)
      .map(|_| ArcRingQueue::new(capacity_per_level))
      .collect();
    Self {
      inner: PriorityQueue::new(levels),
    }
  }

  /// 優先度レベル配列への不変参照を取得します
  ///
  /// # Returns
  ///
  /// 優先度レベルキューのスライス
  pub fn levels(&self) -> &[ArcRingQueue<E>] {
    self.inner.levels()
  }

  /// 優先度レベル配列への可変参照を取得します
  ///
  /// # Returns
  ///
  /// 優先度レベルキューの可変スライス
  pub fn levels_mut(&mut self) -> &mut [ArcRingQueue<E>] {
    self.inner.levels_mut()
  }

  /// 内部優先度キューへの不変参照を取得します
  ///
  /// # Returns
  ///
  /// 内部優先度キューへの参照
  pub fn inner(&self) -> &PriorityQueue<ArcRingQueue<E>, E> {
    &self.inner
  }

  /// 内部優先度キューへの可変参照を取得します
  ///
  /// # Returns
  ///
  /// 内部優先度キューへの可変参照
  pub fn inner_mut(&mut self) -> &mut PriorityQueue<ArcRingQueue<E>, E> {
    &mut self.inner
  }
}

impl<E> ArcPriorityQueue<E>
where
  E: PriorityMessage,
{
  /// 要素を優先度に基づいてキューに追加します
  ///
  /// # Arguments
  ///
  /// * `element` - キューに追加する要素
  ///
  /// # Returns
  ///
  /// 成功時は`Ok(())`、キューが満杯の場合は`Err(QueueError::Full)`
  pub fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer(element)
  }

  /// 最も優先度の高い要素をキューから取り出します
  ///
  /// # Returns
  ///
  /// 成功時は`Ok(Some(E))`、キューが空の場合は`Ok(None)`
  pub fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll()
  }

  /// キュー内のすべての要素をクリアします
  pub fn clean_up(&self) {
    self.inner.clean_up();
  }
}

impl<E> QueueBase<E> for ArcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<E> QueueWriter<E> for ArcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E> QueueReader<E> for ArcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.inner.poll_mut()
  }

  fn clean_up_mut(&mut self) {
    self.inner.clean_up_mut();
  }
}

impl<E> QueueRw<E> for ArcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  fn clean_up(&self) {
    self.clean_up();
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[derive(Debug, Clone)]
  struct Msg(i32, i8);

  impl nexus_utils_core_rs::Element for Msg {}

  impl PriorityMessage for Msg {
    fn get_priority(&self) -> Option<i8> {
      Some(self.1)
    }
  }

  #[test]
  fn priority_queue_orders_elements() {
    let queue = ArcPriorityQueue::new(4);
    queue.offer(Msg(10, 1)).unwrap();
    queue.offer(Msg(99, 7)).unwrap();
    queue.offer(Msg(20, 3)).unwrap();

    assert_eq!(queue.poll().unwrap().unwrap().0, 99);
    assert_eq!(queue.poll().unwrap().unwrap().0, 20);
    assert_eq!(queue.poll().unwrap().unwrap().0, 10);
    assert!(queue.poll().unwrap().is_none());
  }

  #[test]
  fn priority_queue_len_capacity_and_clean_up() {
    let queue = ArcPriorityQueue::new(2);
    assert_eq!(queue.len(), QueueSize::limited(0));

    queue.offer(Msg(1, 0)).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));

    queue.clean_up();
    assert_eq!(queue.len(), QueueSize::limited(0));
  }

  #[test]
  fn priority_queue_capacity_reflects_levels() {
    let queue = ArcPriorityQueue::<Msg>::new(1);
    assert!(queue.capacity().is_limitless());
  }

  #[test]
  fn priority_queue_offer_via_trait() {
    let mut queue = ArcPriorityQueue::new(2);
    queue.offer_mut(Msg(5, 2)).unwrap();
    assert_eq!(queue.poll_mut().unwrap().unwrap().0, 5);
  }
}
