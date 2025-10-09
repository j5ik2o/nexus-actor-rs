use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use nexus_actor_core_rs::{
  Mailbox, MailboxOptions, PriorityEnvelope, QueueMailbox, QueueMailboxProducer, QueueMailboxRecv,
};
use nexus_utils_std_rs::{
  Element, QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter, DEFAULT_CAPACITY, PRIORITY_LEVELS,
};

type PriorityQueueError<M> = Box<QueueError<PriorityEnvelope<M>>>;

use crate::tokio_mailbox::NotifySignal;

struct TokioPriorityLevels<M> {
  levels: Arc<Vec<Mutex<VecDeque<PriorityEnvelope<M>>>>>,
  capacity_per_level: usize,
}

impl<M> Clone for TokioPriorityLevels<M> {
  fn clone(&self) -> Self {
    Self {
      levels: Arc::clone(&self.levels),
      capacity_per_level: self.capacity_per_level,
    }
  }
}

impl<M> TokioPriorityLevels<M> {
  fn new(levels: usize, capacity_per_level: usize) -> Self {
    let storage = (0..levels).map(|_| Mutex::new(VecDeque::new())).collect();
    Self {
      levels: Arc::new(storage),
      capacity_per_level,
    }
  }

  fn level_index(priority: i8, levels: usize) -> usize {
    let max = (levels.saturating_sub(1)) as i8;
    priority.clamp(0, max) as usize
  }

  #[allow(clippy::result_large_err)]
  fn offer(&self, envelope: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    let idx = Self::level_index(envelope.priority(), self.levels.len());
    let mut guard = self.levels[idx].lock().expect("priority queue poisoned");
    if self.capacity_per_level > 0 && guard.len() >= self.capacity_per_level {
      Err(QueueError::Full(envelope))
    } else {
      guard.push_back(envelope);
      Ok(())
    }
  }

  #[allow(clippy::result_large_err)]
  fn poll(&self) -> Result<Option<PriorityEnvelope<M>>, QueueError<PriorityEnvelope<M>>> {
    for level in (0..self.levels.len()).rev() {
      let mut guard = self.levels[level].lock().expect("priority queue poisoned");
      if let Some(envelope) = guard.pop_front() {
        return Ok(Some(envelope));
      }
    }
    Ok(None)
  }

  fn clean_up(&self) {
    for level in self.levels.iter() {
      let mut guard = level.lock().expect("priority queue poisoned");
      guard.clear();
    }
  }

  fn len(&self) -> usize {
    self
      .levels
      .iter()
      .map(|level| level.lock().expect("priority queue poisoned").len())
      .sum()
  }

  fn capacity(&self) -> QueueSize {
    if self.capacity_per_level == 0 {
      QueueSize::limitless()
    } else {
      let levels = self.levels.len().max(1);
      QueueSize::limited(self.capacity_per_level * levels)
    }
  }
}

pub struct TokioPriorityQueues<M> {
  control: TokioPriorityLevels<M>,
  regular: Arc<Mutex<VecDeque<PriorityEnvelope<M>>>>,
  regular_capacity: usize,
}

impl<M> TokioPriorityQueues<M> {
  fn new(levels: usize, control_per_level: usize, regular_capacity: usize) -> Self {
    Self {
      control: TokioPriorityLevels::new(levels, control_per_level),
      regular: Arc::new(Mutex::new(VecDeque::new())),
      regular_capacity,
    }
  }

  #[allow(clippy::result_large_err)]
  fn offer(&self, envelope: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    if envelope.is_control() {
      self.control.offer(envelope)
    } else {
      let mut guard = self.regular.lock().expect("regular queue poisoned");
      if self.regular_capacity > 0 && guard.len() >= self.regular_capacity {
        Err(QueueError::Full(envelope))
      } else {
        guard.push_back(envelope);
        Ok(())
      }
    }
  }

  #[allow(clippy::result_large_err)]
  fn poll(&self) -> Result<Option<PriorityEnvelope<M>>, QueueError<PriorityEnvelope<M>>> {
    if let Some(envelope) = self.control.poll()? {
      return Ok(Some(envelope));
    }
    let mut guard = self.regular.lock().expect("regular queue poisoned");
    Ok(guard.pop_front())
  }

  fn clean_up(&self) {
    self.control.clean_up();
    let mut guard = self.regular.lock().expect("regular queue poisoned");
    guard.clear();
  }

  fn len(&self) -> QueueSize {
    let control_len = self.control.len();
    let regular_len = self.regular.lock().expect("regular queue poisoned").len();
    QueueSize::limited(control_len.saturating_add(regular_len))
  }

  fn capacity(&self) -> QueueSize {
    let control_cap = self.control.capacity();
    let regular_cap = if self.regular_capacity == 0 {
      QueueSize::limitless()
    } else {
      QueueSize::limited(self.regular_capacity)
    };

    if control_cap.is_limitless() || regular_cap.is_limitless() {
      QueueSize::limitless()
    } else {
      let total = control_cap.to_usize().saturating_add(regular_cap.to_usize());
      QueueSize::limited(total)
    }
  }
}

impl<M> Clone for TokioPriorityQueues<M> {
  fn clone(&self) -> Self {
    Self {
      control: self.control.clone(),
      regular: Arc::clone(&self.regular),
      regular_capacity: self.regular_capacity,
    }
  }
}

impl<M> QueueBase<PriorityEnvelope<M>> for TokioPriorityQueues<M> {
  fn len(&self) -> QueueSize {
    self.len()
  }

  fn capacity(&self) -> QueueSize {
    self.capacity()
  }
}

impl<M> QueueWriter<PriorityEnvelope<M>> for TokioPriorityQueues<M> {
  fn offer_mut(&mut self, envelope: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.offer(envelope)
  }
}

impl<M> QueueReader<PriorityEnvelope<M>> for TokioPriorityQueues<M> {
  fn poll_mut(&mut self) -> Result<Option<PriorityEnvelope<M>>, QueueError<PriorityEnvelope<M>>> {
    self.poll()
  }

  fn clean_up_mut(&mut self) {
    self.clean_up();
  }
}

impl<M> QueueRw<PriorityEnvelope<M>> for TokioPriorityQueues<M> {
  fn offer(&self, envelope: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.offer(envelope)
  }

  fn poll(&self) -> Result<Option<PriorityEnvelope<M>>, QueueError<PriorityEnvelope<M>>> {
    self.poll()
  }

  fn clean_up(&self) {
    self.clean_up();
  }
}

/// Tokioランタイム用の優先度付きメールボックス
///
/// メッセージを優先度に基づいて処理する非同期メールボックス。
/// コントロールメッセージは通常メッセージよりも優先的に処理されます。
pub struct TokioPriorityMailbox<M>
where
  M: Element, {
  inner: QueueMailbox<TokioPriorityQueues<M>, NotifySignal>,
}

/// 優先度付きメールボックスへのメッセージ送信ハンドル
///
/// メールボックスにメッセージを送信するための非同期インターフェースを提供します。
/// 優先度を指定したメッセージ送信とコントロールメッセージ送信に対応しています。
pub struct TokioPriorityMailboxSender<M>
where
  M: Element, {
  inner: QueueMailboxProducer<TokioPriorityQueues<M>, NotifySignal>,
}

/// 優先度付きメールボックスを生成するファクトリ
///
/// コントロールキューと通常キューの容量、優先度レベル数を設定し、
/// メールボックスインスタンスを生成します。
#[derive(Clone, Debug)]
pub struct TokioPriorityMailboxFactory {
  control_capacity_per_level: usize,
  regular_capacity: usize,
  levels: usize,
}

impl Default for TokioPriorityMailboxFactory {
  fn default() -> Self {
    Self {
      control_capacity_per_level: DEFAULT_CAPACITY,
      regular_capacity: DEFAULT_CAPACITY,
      levels: PRIORITY_LEVELS,
    }
  }
}

impl TokioPriorityMailboxFactory {
  /// 新しいファクトリインスタンスを作成する
  ///
  /// # 引数
  ///
  /// * `control_capacity_per_level` - コントロールキューの各優先度レベルごとの容量
  ///
  /// # 戻り値
  ///
  /// デフォルトの通常キュー容量とデフォルトの優先度レベル数で初期化されたファクトリ
  pub fn new(control_capacity_per_level: usize) -> Self {
    Self {
      control_capacity_per_level,
      regular_capacity: DEFAULT_CAPACITY,
      levels: PRIORITY_LEVELS,
    }
  }

  /// 優先度レベル数を設定する（ビルダーパターン）
  ///
  /// # 引数
  ///
  /// * `levels` - 設定する優先度レベル数（最小値は1）
  ///
  /// # 戻り値
  ///
  /// 設定を更新したファクトリインスタンス
  pub fn with_levels(mut self, levels: usize) -> Self {
    self.levels = levels.max(1);
    self
  }

  /// 通常キューの容量を設定する（ビルダーパターン）
  ///
  /// # 引数
  ///
  /// * `capacity` - 通常メッセージキューの容量
  ///
  /// # 戻り値
  ///
  /// 設定を更新したファクトリインスタンス
  pub fn with_regular_capacity(mut self, capacity: usize) -> Self {
    self.regular_capacity = capacity;
    self
  }

  /// メールボックスと送信ハンドルのペアを生成する
  ///
  /// # 引数
  ///
  /// * `options` - メールボックスの容量オプション
  ///
  /// # 戻り値
  ///
  /// `(TokioPriorityMailbox<M>, TokioPriorityMailboxSender<M>)` - メールボックスと送信ハンドルのタプル
  pub fn mailbox<M>(&self, options: MailboxOptions) -> (TokioPriorityMailbox<M>, TokioPriorityMailboxSender<M>)
  where
    M: Element, {
    let control_per_level = self.resolve_control_capacity(options.priority_capacity);
    let regular_capacity = self.resolve_regular_capacity(options.capacity);
    let queue = TokioPriorityQueues::<M>::new(self.levels, control_per_level, regular_capacity);
    let signal = NotifySignal::default();
    let mailbox = QueueMailbox::new(queue, signal);
    let sender = mailbox.producer();
    (
      TokioPriorityMailbox { inner: mailbox },
      TokioPriorityMailboxSender { inner: sender },
    )
  }

  fn resolve_control_capacity(&self, requested: QueueSize) -> usize {
    match requested {
      QueueSize::Limitless => self.control_capacity_per_level,
      QueueSize::Limited(value) => value,
    }
  }

  fn resolve_regular_capacity(&self, requested: QueueSize) -> usize {
    match requested {
      QueueSize::Limitless => self.regular_capacity,
      QueueSize::Limited(value) => value,
    }
  }
}

impl<M> TokioPriorityMailbox<M>
where
  M: Element,
{
  /// 新しい優先度付きメールボックスを作成する
  ///
  /// # 引数
  ///
  /// * `control_capacity_per_level` - コントロールキューの各優先度レベルごとの容量
  ///
  /// # 戻り値
  ///
  /// `(TokioPriorityMailbox<M>, TokioPriorityMailboxSender<M>)` - メールボックスと送信ハンドルのタプル
  pub fn new(control_capacity_per_level: usize) -> (Self, TokioPriorityMailboxSender<M>) {
    TokioPriorityMailboxFactory::new(control_capacity_per_level).mailbox::<M>(MailboxOptions::default())
  }

  /// 内部の`QueueMailbox`への参照を取得する
  ///
  /// # 戻り値
  ///
  /// 内部メールボックスへの不変参照
  pub fn inner(&self) -> &QueueMailbox<TokioPriorityQueues<M>, NotifySignal> {
    &self.inner
  }
}

impl<M> Mailbox<PriorityEnvelope<M>> for TokioPriorityMailbox<M>
where
  M: Element,
{
  type RecvFuture<'a>
    = QueueMailboxRecv<'a, TokioPriorityQueues<M>, NotifySignal, PriorityEnvelope<M>>
  where
    Self: 'a;
  type SendError = PriorityQueueError<M>;

  fn try_send(&self, message: PriorityEnvelope<M>) -> Result<(), Self::SendError> {
    self.inner.try_send(message).map_err(Box::new)
  }

  fn recv(&self) -> Self::RecvFuture<'_> {
    self.inner.recv()
  }

  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }

  fn close(&self) {
    self.inner.close();
  }

  fn is_closed(&self) -> bool {
    self.inner.is_closed()
  }
}

impl<M> TokioPriorityMailboxSender<M>
where
  M: Element,
{
  /// メッセージを非ブロッキングで送信する
  ///
  /// # 引数
  ///
  /// * `message` - 送信する優先度付きエンベロープ
  ///
  /// # 戻り値
  ///
  /// メッセージが正常にキューに追加された場合は`Ok(())`、キューが満杯の場合は`Err`
  ///
  /// # エラー
  ///
  /// キューが満杯の場合、または送信に失敗した場合
  pub fn try_send(&self, message: PriorityEnvelope<M>) -> Result<(), PriorityQueueError<M>> {
    self.inner.try_send(message).map_err(Box::new)
  }

  /// メッセージを非同期で送信する
  ///
  /// キューに空きができるまで待機します。
  ///
  /// # 引数
  ///
  /// * `message` - 送信する優先度付きエンベロープ
  ///
  /// # 戻り値
  ///
  /// メッセージが正常に送信された場合は`Ok(())`、失敗した場合は`Err`
  ///
  /// # エラー
  ///
  /// 送信に失敗した場合
  pub async fn send(&self, message: PriorityEnvelope<M>) -> Result<(), PriorityQueueError<M>> {
    self.inner.send(message).await.map_err(Box::new)
  }

  /// 優先度を指定してメッセージを非ブロッキングで送信する
  ///
  /// # 引数
  ///
  /// * `message` - 送信するメッセージ
  /// * `priority` - メッセージの優先度
  ///
  /// # 戻り値
  ///
  /// メッセージが正常にキューに追加された場合は`Ok(())`、失敗した場合は`Err`
  ///
  /// # エラー
  ///
  /// キューが満杯の場合、または送信に失敗した場合
  pub fn try_send_with_priority(&self, message: M, priority: i8) -> Result<(), PriorityQueueError<M>> {
    self.try_send(PriorityEnvelope::new(message, priority))
  }

  /// 優先度を指定してメッセージを非同期で送信する
  ///
  /// # 引数
  ///
  /// * `message` - 送信するメッセージ
  /// * `priority` - メッセージの優先度
  ///
  /// # 戻り値
  ///
  /// メッセージが正常に送信された場合は`Ok(())`、失敗した場合は`Err`
  ///
  /// # エラー
  ///
  /// 送信に失敗した場合
  pub async fn send_with_priority(&self, message: M, priority: i8) -> Result<(), PriorityQueueError<M>> {
    self.send(PriorityEnvelope::new(message, priority)).await
  }

  /// コントロールメッセージを優先度付きで非ブロッキング送信する
  ///
  /// コントロールメッセージは通常メッセージより優先的に処理されます。
  ///
  /// # 引数
  ///
  /// * `message` - 送信するメッセージ
  /// * `priority` - メッセージの優先度
  ///
  /// # 戻り値
  ///
  /// メッセージが正常にキューに追加された場合は`Ok(())`、失敗した場合は`Err`
  ///
  /// # エラー
  ///
  /// キューが満杯の場合、または送信に失敗した場合
  pub fn try_send_control_with_priority(&self, message: M, priority: i8) -> Result<(), PriorityQueueError<M>> {
    self.try_send(PriorityEnvelope::control(message, priority))
  }

  /// コントロールメッセージを優先度付きで非同期送信する
  ///
  /// コントロールメッセージは通常メッセージより優先的に処理されます。
  ///
  /// # 引数
  ///
  /// * `message` - 送信するメッセージ
  /// * `priority` - メッセージの優先度
  ///
  /// # 戻り値
  ///
  /// メッセージが正常に送信された場合は`Ok(())`、失敗した場合は`Err`
  ///
  /// # エラー
  ///
  /// 送信に失敗した場合
  pub async fn send_control_with_priority(&self, message: M, priority: i8) -> Result<(), PriorityQueueError<M>> {
    self.send(PriorityEnvelope::control(message, priority)).await
  }

  /// 内部の`QueueMailboxProducer`への参照を取得する
  ///
  /// # 戻り値
  ///
  /// 内部プロデューサーへの不変参照
  pub fn inner(&self) -> &QueueMailboxProducer<TokioPriorityQueues<M>, NotifySignal> {
    &self.inner
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use nexus_utils_std_rs::{QueueSize, DEFAULT_PRIORITY};

  async fn run_priority_runtime_orders_messages() {
    let factory = TokioPriorityMailboxFactory::default();
    let (mailbox, sender) = factory.mailbox::<u32>(MailboxOptions::default());

    sender
      .send_with_priority(10, DEFAULT_PRIORITY)
      .await
      .expect("send low priority");
    sender
      .send_control_with_priority(99, DEFAULT_PRIORITY + 7)
      .await
      .expect("send high priority");
    sender
      .send_control_with_priority(20, DEFAULT_PRIORITY + 3)
      .await
      .expect("send medium priority");

    tokio::task::yield_now().await;

    let first = mailbox.recv().await.expect("first message");
    let second = mailbox.recv().await.expect("second message");
    let third = mailbox.recv().await.expect("third message");

    assert_eq!(first.into_parts(), (99, DEFAULT_PRIORITY + 7));
    assert_eq!(second.into_parts(), (20, DEFAULT_PRIORITY + 3));
    assert_eq!(third.into_parts(), (10, DEFAULT_PRIORITY));
  }

  #[tokio::test(flavor = "current_thread")]
  async fn priority_runtime_orders_messages() {
    run_priority_runtime_orders_messages().await;
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn priority_runtime_orders_messages_multi_thread() {
    run_priority_runtime_orders_messages().await;
  }

  async fn run_priority_sender_defaults_work() {
    let factory = TokioPriorityMailboxFactory::new(4).with_regular_capacity(4);
    let (mailbox, sender) = factory.mailbox::<u8>(MailboxOptions::default());

    sender
      .send(PriorityEnvelope::with_default_priority(5))
      .await
      .expect("send default priority");

    let envelope = mailbox.recv().await.expect("receive envelope");
    let (_, priority) = envelope.into_parts();
    assert_eq!(priority, DEFAULT_PRIORITY);
  }

  #[tokio::test(flavor = "current_thread")]
  async fn priority_sender_defaults_work() {
    run_priority_sender_defaults_work().await;
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn priority_sender_defaults_work_multi_thread() {
    run_priority_sender_defaults_work().await;
  }

  async fn run_control_queue_preempts_regular_messages() {
    let factory = TokioPriorityMailboxFactory::default();
    let (mailbox, sender) = factory.mailbox::<u32>(MailboxOptions::default());

    sender
      .send_with_priority(1, DEFAULT_PRIORITY)
      .await
      .expect("enqueue regular message");
    sender
      .send_control_with_priority(99, DEFAULT_PRIORITY + 5)
      .await
      .expect("enqueue control message");

    let first = mailbox.recv().await.expect("first message");
    let second = mailbox.recv().await.expect("second message");

    assert_eq!(first.into_parts(), (99, DEFAULT_PRIORITY + 5));
    assert_eq!(second.into_parts(), (1, DEFAULT_PRIORITY));
  }

  #[tokio::test(flavor = "current_thread")]
  async fn control_queue_preempts_regular_messages() {
    run_control_queue_preempts_regular_messages().await;
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn control_queue_preempts_regular_messages_multi_thread() {
    run_control_queue_preempts_regular_messages().await;
  }

  async fn run_priority_mailbox_capacity_split() {
    let factory = TokioPriorityMailboxFactory::default();
    let options = MailboxOptions::with_capacities(QueueSize::limited(2), QueueSize::limited(2));
    let (mailbox, sender) = factory.mailbox::<u8>(options);

    assert!(!mailbox.capacity().is_limitless());

    sender
      .send_control_with_priority(1, DEFAULT_PRIORITY + 2)
      .await
      .expect("control enqueue");
    sender
      .send_with_priority(2, DEFAULT_PRIORITY)
      .await
      .expect("regular enqueue");
    sender
      .send_with_priority(3, DEFAULT_PRIORITY)
      .await
      .expect("second regular enqueue");

    let err = sender
      .try_send_with_priority(4, DEFAULT_PRIORITY)
      .expect_err("regular capacity reached");
    assert!(matches!(&*err, QueueError::Full(_)));
  }

  #[tokio::test(flavor = "current_thread")]
  async fn priority_mailbox_capacity_split() {
    run_priority_mailbox_capacity_split().await;
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn priority_mailbox_capacity_split_multi_thread() {
    run_priority_mailbox_capacity_split().await;
  }
}
