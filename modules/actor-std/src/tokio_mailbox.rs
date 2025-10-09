use std::sync::Arc;

use nexus_actor_core_rs::{
  Mailbox, MailboxFactory, MailboxOptions, MailboxPair, MailboxSignal, QueueMailbox, QueueMailboxProducer,
  QueueMailboxRecv,
};
use nexus_utils_std_rs::{ArcMpscBoundedQueue, ArcMpscUnboundedQueue};
use nexus_utils_std_rs::{Element, QueueBase, QueueError, QueueRw, QueueSize};
use tokio::sync::{futures::Notified, Notify};

/// Tokioランタイム用のメールボックス実装
///
/// アクターへのメッセージ配信を管理する非同期キューです。
#[derive(Clone, Debug)]
pub struct TokioMailbox<M>
where
  M: Element, {
  inner: QueueMailbox<TokioQueue<M>, NotifySignal>,
}

/// Tokioメールボックスへの送信側ハンドル
///
/// メッセージの送信に特化したインターフェースを提供します。
#[derive(Clone, Debug)]
pub struct TokioMailboxSender<M>
where
  M: Element, {
  inner: QueueMailboxProducer<TokioQueue<M>, NotifySignal>,
}

/// Tokioメールボックスを生成するファクトリ
///
/// 有界・無界のメールボックスを生成します。
#[derive(Clone, Debug, Default)]
pub struct TokioMailboxFactory;

#[derive(Clone, Debug)]
pub struct NotifySignal {
  inner: Arc<Notify>,
}

impl Default for NotifySignal {
  fn default() -> Self {
    Self {
      inner: Arc::new(Notify::new()),
    }
  }
}

impl MailboxSignal for NotifySignal {
  type WaitFuture<'a>
    = Notified<'a>
  where
    Self: 'a;

  fn notify(&self) {
    self.inner.notify_one();
  }

  fn wait(&self) -> Self::WaitFuture<'_> {
    self.inner.notified()
  }
}

#[derive(Debug)]
pub struct TokioQueue<M>
where
  M: Element, {
  inner: Arc<TokioQueueKind<M>>,
}

#[derive(Debug)]
enum TokioQueueKind<M>
where
  M: Element, {
  Unbounded(ArcMpscUnboundedQueue<M>),
  Bounded(ArcMpscBoundedQueue<M>),
}

impl<M> Clone for TokioQueue<M>
where
  M: Element,
{
  fn clone(&self) -> Self {
    Self {
      inner: Arc::clone(&self.inner),
    }
  }
}

impl<M> TokioQueue<M>
where
  M: Element,
{
  fn with_capacity(size: QueueSize) -> Self {
    let kind = match size {
      QueueSize::Limitless => TokioQueueKind::Unbounded(ArcMpscUnboundedQueue::new()),
      QueueSize::Limited(0) => TokioQueueKind::Unbounded(ArcMpscUnboundedQueue::new()),
      QueueSize::Limited(capacity) => TokioQueueKind::Bounded(ArcMpscBoundedQueue::new(capacity)),
    };
    Self { inner: Arc::new(kind) }
  }

  fn kind(&self) -> &TokioQueueKind<M> {
    self.inner.as_ref()
  }
}

impl<M> QueueBase<M> for TokioQueue<M>
where
  M: Element,
{
  fn len(&self) -> QueueSize {
    match self.kind() {
      TokioQueueKind::Unbounded(queue) => queue.len(),
      TokioQueueKind::Bounded(queue) => queue.len(),
    }
  }

  fn capacity(&self) -> QueueSize {
    match self.kind() {
      TokioQueueKind::Unbounded(queue) => queue.capacity(),
      TokioQueueKind::Bounded(queue) => queue.capacity(),
    }
  }
}

impl<M> QueueRw<M> for TokioQueue<M>
where
  M: Element,
{
  fn offer(&self, element: M) -> Result<(), QueueError<M>> {
    match self.kind() {
      TokioQueueKind::Unbounded(queue) => queue.offer(element),
      TokioQueueKind::Bounded(queue) => queue.offer(element),
    }
  }

  fn poll(&self) -> Result<Option<M>, QueueError<M>> {
    match self.kind() {
      TokioQueueKind::Unbounded(queue) => queue.poll(),
      TokioQueueKind::Bounded(queue) => queue.poll(),
    }
  }

  fn clean_up(&self) {
    match self.kind() {
      TokioQueueKind::Unbounded(queue) => queue.clean_up(),
      TokioQueueKind::Bounded(queue) => queue.clean_up(),
    }
  }
}

impl TokioMailboxFactory {
  /// 指定されたオプションでメールボックスを生成する
  ///
  /// # 引数
  /// * `options` - メールボックスの設定オプション
  ///
  /// # 戻り値
  /// メールボックスと送信側ハンドルのペア
  pub fn mailbox<M>(&self, options: MailboxOptions) -> (TokioMailbox<M>, TokioMailboxSender<M>)
  where
    M: Element, {
    let (mailbox, sender) = self.build_mailbox::<M>(options);
    (TokioMailbox { inner: mailbox }, TokioMailboxSender { inner: sender })
  }

  /// 指定された容量の有界メールボックスを生成する
  ///
  /// # 引数
  /// * `capacity` - メールボックスの最大容量
  ///
  /// # 戻り値
  /// メールボックスと送信側ハンドルのペア
  pub fn with_capacity<M>(&self, capacity: usize) -> (TokioMailbox<M>, TokioMailboxSender<M>)
  where
    M: Element, {
    self.mailbox(MailboxOptions::with_capacity(capacity))
  }

  /// 無界メールボックスを生成する
  ///
  /// # 戻り値
  /// メールボックスと送信側ハンドルのペア
  pub fn unbounded<M>(&self) -> (TokioMailbox<M>, TokioMailboxSender<M>)
  where
    M: Element, {
    self.mailbox(MailboxOptions::unbounded())
  }
}

impl MailboxFactory for TokioMailboxFactory {
  type Queue<M>
    = TokioQueue<M>
  where
    M: Element;
  type Signal = NotifySignal;

  fn build_mailbox<M>(&self, options: MailboxOptions) -> MailboxPair<Self::Queue<M>, Self::Signal>
  where
    M: Element, {
    let queue = TokioQueue::with_capacity(options.capacity);
    let signal = NotifySignal::default();
    let mailbox = QueueMailbox::new(queue, signal);
    let sender = mailbox.producer();
    (mailbox, sender)
  }
}

impl<M> TokioMailbox<M>
where
  M: Element,
{
  /// 指定容量のメールボックスを生成する
  ///
  /// # 引数
  /// * `capacity` - メールボックスの最大容量
  ///
  /// # 戻り値
  /// メールボックスと送信側ハンドルのペア
  pub fn new(capacity: usize) -> (Self, TokioMailboxSender<M>) {
    TokioMailboxFactory.with_capacity(capacity)
  }

  /// 無界メールボックスを生成する
  ///
  /// # 戻り値
  /// メールボックスと送信側ハンドルのペア
  pub fn unbounded() -> (Self, TokioMailboxSender<M>) {
    TokioMailboxFactory.unbounded()
  }

  /// 新しい送信側ハンドルを生成する
  ///
  /// # 戻り値
  /// メッセージ送信用の`TokioMailboxSender`
  pub fn producer(&self) -> TokioMailboxSender<M>
  where
    TokioQueue<M>: Clone,
    NotifySignal: Clone, {
    TokioMailboxSender {
      inner: self.inner.producer(),
    }
  }

  /// 内部のキューメールボックスへの参照を取得する
  ///
  /// # 戻り値
  /// 内部メールボックスへの不変参照
  pub fn inner(&self) -> &QueueMailbox<TokioQueue<M>, NotifySignal> {
    &self.inner
  }
}

impl<M> Mailbox<M> for TokioMailbox<M>
where
  M: Element,
  TokioQueue<M>: Clone,
{
  type RecvFuture<'a>
    = QueueMailboxRecv<'a, TokioQueue<M>, NotifySignal, M>
  where
    Self: 'a;
  type SendError = QueueError<M>;

  fn try_send(&self, message: M) -> Result<(), Self::SendError> {
    self.inner.try_send(message)
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

impl<M> TokioMailboxSender<M>
where
  M: Element,
  TokioQueue<M>: Clone,
{
  /// メッセージの送信を試みる（ブロックしない）
  ///
  /// # 引数
  /// * `message` - 送信するメッセージ
  ///
  /// # 戻り値
  /// 成功時は`Ok(())`、失敗時はエラーとメッセージを返す
  ///
  /// # エラー
  /// キューが満杯の場合、`QueueError::Full`を返す
  pub fn try_send(&self, message: M) -> Result<(), QueueError<M>> {
    self.inner.try_send(message)
  }

  /// メッセージを非同期で送信する
  ///
  /// # 引数
  /// * `message` - 送信するメッセージ
  ///
  /// # 戻り値
  /// 成功時は`Ok(())`、失敗時はエラーとメッセージを返す
  ///
  /// # エラー
  /// メールボックスが閉じられている場合、`QueueError::Closed`を返す
  pub async fn send(&self, message: M) -> Result<(), QueueError<M>> {
    self.inner.send(message).await
  }

  /// 内部のキューメールボックスプロデューサーへの参照を取得する
  ///
  /// # 戻り値
  /// 内部プロデューサーへの不変参照
  pub fn inner(&self) -> &QueueMailboxProducer<TokioQueue<M>, NotifySignal> {
    &self.inner
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use nexus_utils_std_rs::QueueError;

  async fn run_runtime_with_capacity_enforces_bounds() {
    let factory = TokioMailboxFactory;
    let (mailbox, sender) = factory.with_capacity::<u32>(2);

    sender.try_send(1).expect("first message accepted");
    sender.try_send(2).expect("second message accepted");
    assert!(matches!(sender.try_send(3), Err(QueueError::Full(3))));
    assert_eq!(mailbox.len().to_usize(), 2);

    let first = mailbox.recv().await.expect("first message");
    let second = mailbox.recv().await.expect("second message");

    assert_eq!(first, 1);
    assert_eq!(second, 2);
    assert_eq!(mailbox.len().to_usize(), 0);
  }

  #[tokio::test(flavor = "current_thread")]
  async fn runtime_with_capacity_enforces_bounds() {
    run_runtime_with_capacity_enforces_bounds().await;
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn runtime_with_capacity_enforces_bounds_multi_thread() {
    run_runtime_with_capacity_enforces_bounds().await;
  }

  async fn run_runtime_unbounded_mailbox_accepts_multiple_messages() {
    let factory = TokioMailboxFactory;
    let (mailbox, sender) = factory.unbounded::<u32>();

    for value in 0..32_u32 {
      sender.send(value).await.expect("send succeeds");
    }

    assert!(mailbox.capacity().is_limitless());

    for expected in 0..32_u32 {
      let received = mailbox.recv().await.expect("receive message");
      assert_eq!(received, expected);
    }

    assert_eq!(mailbox.len().to_usize(), 0);
  }

  #[tokio::test(flavor = "current_thread")]
  async fn runtime_unbounded_mailbox_accepts_multiple_messages() {
    run_runtime_unbounded_mailbox_accepts_multiple_messages().await;
  }

  #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
  async fn runtime_unbounded_mailbox_accepts_multiple_messages_multi_thread() {
    run_runtime_unbounded_mailbox_accepts_multiple_messages().await;
  }
}
