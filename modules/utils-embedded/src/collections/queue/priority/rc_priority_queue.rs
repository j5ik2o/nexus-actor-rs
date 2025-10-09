use nexus_utils_core_rs::{
  PriorityMessage, PriorityQueue, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter, PRIORITY_LEVELS,
};

use crate::RcRingQueue;

/// `Rc`ベースの優先度付きキュー
///
/// このキューは`no_std`環境で利用可能な、メッセージの優先度に基づいて
/// 処理順序を制御するキューです。`Rc`と`RefCell`を使用して
/// 参照カウントベースの共有所有権を提供します。
///
/// # 特徴
///
/// - **優先度ベース**: メッセージの優先度（0-7）に基づいて処理順序を決定
/// - **複数レベル**: 8つの優先度レベルをサポート
/// - **動的/静的モード**: 動的拡張または固定容量のいずれかを選択可能
/// - **no_std対応**: 標準ライブラリを必要としません
/// - **クローン可能**: `clone()`で複数のハンドルを作成可能
///
/// # 優先度
///
/// - 優先度は0（最低）から7（最高）までの8レベル
/// - 優先度が指定されていない場合、デフォルト優先度（0）が使用されます
/// - 高い優先度のメッセージが先に処理されます
///
/// # パフォーマンス特性
///
/// - `offer`: O(1)
/// - `poll`: O(PRIORITY_LEVELS)、通常はO(1)に近い
/// - メモリ使用量: O(capacity_per_level * PRIORITY_LEVELS)
///
/// # 例
///
/// ```
/// use nexus_utils_embedded_rs::RcPriorityQueue;
/// use nexus_utils_core_rs::{QueueRw, PriorityMessage};
///
/// #[derive(Debug)]
/// struct Task {
///     id: u32,
///     priority: i8,
/// }
///
/// impl nexus_utils_core_rs::Element for Task {}
///
/// impl PriorityMessage for Task {
///     fn get_priority(&self) -> Option<i8> {
///         Some(self.priority)
///     }
/// }
///
/// let queue = RcPriorityQueue::new(10);
/// queue.offer(Task { id: 1, priority: 0 }).unwrap();
/// queue.offer(Task { id: 2, priority: 5 }).unwrap();
///
/// // 高優先度のタスクが先に取得される
/// let task = queue.poll().unwrap().unwrap();
/// assert_eq!(task.id, 2);
/// ```
#[derive(Debug, Clone)]
pub struct RcPriorityQueue<E> {
  inner: PriorityQueue<RcRingQueue<E>, E>,
}

impl<E> RcPriorityQueue<E> {
  /// 各優先度レベルに対して指定された容量で新しい優先度付きキューを作成します
  ///
  /// # 引数
  ///
  /// * `capacity_per_level` - 各優先度レベルのキューに格納できる最大要素数
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcPriorityQueue;
  ///
  /// // 各優先度レベルに10要素まで格納可能
  /// let queue: RcPriorityQueue<u32> = RcPriorityQueue::new(10);
  /// ```
  pub fn new(capacity_per_level: usize) -> Self {
    let levels = (0..PRIORITY_LEVELS)
      .map(|_| RcRingQueue::new(capacity_per_level))
      .collect();
    Self {
      inner: PriorityQueue::new(levels),
    }
  }

  /// キューの動的拡張モードを設定します
  ///
  /// # 引数
  ///
  /// * `dynamic` - `true`の場合、容量が不足すると自動的に拡張されます。
  ///               `false`の場合、容量制限が厳密に適用されます。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcPriorityQueue;
  ///
  /// let queue: RcPriorityQueue<i32> = RcPriorityQueue::new(5);
  /// queue.set_dynamic(false); // 固定容量モード
  /// ```
  pub fn set_dynamic(&self, dynamic: bool) {
    for queue in self.inner.levels() {
      queue.set_dynamic(dynamic);
    }
  }

  /// 動的拡張モードを設定して自身を返します（ビルダーパターン）
  ///
  /// # 引数
  ///
  /// * `dynamic` - `true`の場合、容量が不足すると自動的に拡張されます
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcPriorityQueue;
  ///
  /// let queue: RcPriorityQueue<i32> = RcPriorityQueue::new(5)
  ///     .with_dynamic(false);
  /// ```
  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  /// 内部の優先度レベル別キューへの不変参照を返します
  ///
  /// # 戻り値
  ///
  /// 8つの優先度レベルキューの配列への参照
  pub fn levels(&self) -> &[RcRingQueue<E>] {
    self.inner.levels()
  }

  /// 内部の優先度レベル別キューへの可変参照を返します
  ///
  /// # 戻り値
  ///
  /// 8つの優先度レベルキューの配列への可変参照
  pub fn levels_mut(&mut self) -> &mut [RcRingQueue<E>] {
    self.inner.levels_mut()
  }

  /// 内部の`PriorityQueue`への不変参照を返します
  ///
  /// # 戻り値
  ///
  /// 内部の`PriorityQueue`インスタンスへの参照
  pub fn inner(&self) -> &PriorityQueue<RcRingQueue<E>, E> {
    &self.inner
  }
}

impl<E> QueueBase<E> for RcPriorityQueue<E>
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

impl<E> QueueWriter<E> for RcPriorityQueue<E>
where
  E: PriorityMessage,
{
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.inner.offer_mut(element)
  }
}

impl<E> QueueReader<E> for RcPriorityQueue<E>
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

impl<E> QueueRw<E> for RcPriorityQueue<E>
where
  E: PriorityMessage,
{
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
  use nexus_utils_core_rs::{QueueBase, QueueReader, QueueWriter};

  #[derive(Debug, Clone)]
  struct Msg(i32, i8);

  impl nexus_utils_core_rs::Element for Msg {}

  impl PriorityMessage for Msg {
    fn get_priority(&self) -> Option<i8> {
      Some(self.1)
    }
  }

  #[test]
  fn rc_priority_queue_orders() {
    let queue: RcPriorityQueue<Msg> = RcPriorityQueue::new(4);
    queue.offer(Msg(1, 0)).unwrap();
    queue.offer(Msg(5, 7)).unwrap();
    queue.offer(Msg(3, 3)).unwrap();

    assert_eq!(queue.poll().unwrap().unwrap().0, 5);
    assert_eq!(queue.poll().unwrap().unwrap().0, 3);
    assert_eq!(queue.poll().unwrap().unwrap().0, 1);
    assert!(queue.poll().unwrap().is_none());
  }

  #[test]
  fn rc_priority_queue_len_capacity_updates() {
    let queue: RcPriorityQueue<Msg> = RcPriorityQueue::new(2);
    assert_eq!(queue.len(), QueueSize::limited(0));

    queue.offer(Msg(1, 0)).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(1));

    queue.clean_up();
    assert_eq!(queue.len(), QueueSize::limited(0));
  }

  #[test]
  fn rc_priority_queue_len_across_levels() {
    let queue: RcPriorityQueue<Msg> = RcPriorityQueue::new(2);
    queue.offer(Msg(1, 0)).unwrap();
    queue.offer(Msg(2, 5)).unwrap();
    assert_eq!(queue.len(), QueueSize::limited(2));
  }

  #[test]
  fn rc_priority_queue_capacity_behaviour() {
    let queue: RcPriorityQueue<Msg> = RcPriorityQueue::new(1);
    assert!(queue.capacity().is_limitless());

    queue.set_dynamic(false);
    let expected = QueueSize::limited(PRIORITY_LEVELS);
    assert_eq!(queue.capacity(), expected);
  }

  #[test]
  fn rc_priority_queue_trait_cleanup() {
    let mut queue: RcPriorityQueue<Msg> = RcPriorityQueue::new(2).with_dynamic(false);
    queue.offer_mut(Msg(1, 0)).unwrap();
    queue.offer_mut(Msg(2, 1)).unwrap();
    assert_eq!(queue.poll_mut().unwrap().unwrap().0, 2);
    queue.clean_up_mut();
    assert!(queue.poll_mut().unwrap().is_none());
  }

  #[test]
  fn rc_priority_queue_priority_clamp_and_default() {
    #[derive(Debug, Clone)]
    struct OptionalPriority(i32, Option<i8>);

    impl nexus_utils_core_rs::Element for OptionalPriority {}

    impl PriorityMessage for OptionalPriority {
      fn get_priority(&self) -> Option<i8> {
        self.1
      }
    }

    let queue: RcPriorityQueue<OptionalPriority> = RcPriorityQueue::new(1).with_dynamic(false);
    queue.offer(OptionalPriority(1, Some(127))).unwrap();
    queue.offer(OptionalPriority(2, Some(-128))).unwrap();
    queue.offer(OptionalPriority(3, None)).unwrap();

    assert_eq!(queue.poll().unwrap().unwrap().0, 1);
    assert_eq!(queue.poll().unwrap().unwrap().0, 3);
    assert_eq!(queue.poll().unwrap().unwrap().0, 2);
  }
}
