use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll};

use nexus_utils_core_rs::Flag;
use nexus_utils_core_rs::{Element, QueueError, QueueRw, QueueSize};

use super::traits::{Mailbox, MailboxSignal};

/// Runtime-agnostic construction options for [`QueueMailbox`].
///
/// メールボックスの容量設定を保持します。
/// 通常のメッセージと優先度付きメッセージで異なる容量を設定できます。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxOptions {
  /// 通常のメッセージキューの容量
  pub capacity: QueueSize,
  /// 優先度付きメッセージキューの容量
  pub priority_capacity: QueueSize,
}

impl MailboxOptions {
  /// 指定された容量でメールボックスオプションを作成します。
  ///
  /// 優先度付きメッセージキューは無制限になります。
  ///
  /// # Arguments
  /// - `capacity`: 通常のメッセージキューの容量
  pub const fn with_capacity(capacity: usize) -> Self {
    Self {
      capacity: QueueSize::limited(capacity),
      priority_capacity: QueueSize::limitless(),
    }
  }

  /// 通常と優先度付きの両方の容量を指定してメールボックスオプションを作成します。
  ///
  /// # Arguments
  /// - `capacity`: 通常のメッセージキューの容量
  /// - `priority_capacity`: 優先度付きメッセージキューの容量
  pub const fn with_capacities(capacity: QueueSize, priority_capacity: QueueSize) -> Self {
    Self {
      capacity,
      priority_capacity,
    }
  }

  /// 優先度付きメッセージキューの容量を設定します。
  ///
  /// # Arguments
  /// - `priority_capacity`: 優先度付きメッセージキューの容量
  pub const fn with_priority_capacity(mut self, priority_capacity: QueueSize) -> Self {
    self.priority_capacity = priority_capacity;
    self
  }

  /// 無制限の容量でメールボックスオプションを作成します。
  pub const fn unbounded() -> Self {
    Self {
      capacity: QueueSize::limitless(),
      priority_capacity: QueueSize::limitless(),
    }
  }
}

impl Default for MailboxOptions {
  fn default() -> Self {
    Self::unbounded()
  }
}

/// Mailbox implementation backed by a generic queue and notification signal.
///
/// ジェネリックなキューと通知シグナルに基づくメールボックス実装です。
/// 非同期ランタイムに依存しない汎用的な設計になっています。
///
/// # 型パラメータ
/// - `Q`: メッセージキューの実装型
/// - `S`: 通知シグナルの実装型
#[derive(Debug)]
pub struct QueueMailbox<Q, S> {
  queue: Q,
  signal: S,
  closed: Flag,
}

impl<Q, S> QueueMailbox<Q, S> {
  /// 新しいキューメールボックスを作成します。
  ///
  /// # Arguments
  /// - `queue`: メッセージキューの実装
  /// - `signal`: 通知シグナルの実装
  pub fn new(queue: Q, signal: S) -> Self {
    Self {
      queue,
      signal,
      closed: Flag::default(),
    }
  }

  /// 内部のキューへの参照を取得します。
  pub fn queue(&self) -> &Q {
    &self.queue
  }

  /// 内部のシグナルへの参照を取得します。
  pub fn signal(&self) -> &S {
    &self.signal
  }

  /// メッセージ送信用のプロデューサーハンドルを作成します。
  ///
  /// プロデューサーは複数のスレッドで共有でき、メールボックスへのメッセージ送信に使用されます。
  pub fn producer(&self) -> QueueMailboxProducer<Q, S>
  where
    Q: Clone,
    S: Clone, {
    QueueMailboxProducer {
      queue: self.queue.clone(),
      signal: self.signal.clone(),
      closed: self.closed.clone(),
    }
  }
}

impl<Q, S> Clone for QueueMailbox<Q, S>
where
  Q: Clone,
  S: Clone,
{
  fn clone(&self) -> Self {
    Self {
      queue: self.queue.clone(),
      signal: self.signal.clone(),
      closed: self.closed.clone(),
    }
  }
}

/// Sending handle that shares queue ownership with [`QueueMailbox`].
///
/// メールボックスとキューの所有権を共有する送信ハンドルです。
/// 複数のスレッドから安全にメッセージを送信できます。
///
/// # 型パラメータ
/// - `Q`: メッセージキューの実装型
/// - `S`: 通知シグナルの実装型
#[derive(Clone, Debug)]
pub struct QueueMailboxProducer<Q, S> {
  queue: Q,
  signal: S,
  closed: Flag,
}

unsafe impl<Q, S> Send for QueueMailboxProducer<Q, S>
where
  Q: Send + Sync,
  S: Send + Sync,
{
}

unsafe impl<Q, S> Sync for QueueMailboxProducer<Q, S>
where
  Q: Send + Sync,
  S: Send + Sync,
{
}

impl<Q, S> QueueMailboxProducer<Q, S> {
  /// メッセージの送信を試みます（ブロッキングなし）。
  ///
  /// キューが満杯の場合は即座にエラーを返します。
  ///
  /// # Arguments
  /// - `message`: 送信するメッセージ
  ///
  /// # Returns
  /// 成功時は `Ok(())`、失敗時は `Err(QueueError)`
  ///
  /// # Errors
  /// - `QueueError::Disconnected`: メールボックスが閉じられている
  /// - `QueueError::Full`: キューが満杯
  pub fn try_send<M>(&self, message: M) -> Result<(), QueueError<M>>
  where
    Q: QueueRw<M>,
    S: MailboxSignal,
    M: Element, {
    if self.closed.get() {
      return Err(QueueError::Disconnected);
    }

    match self.queue.offer(message) {
      Ok(()) => {
        self.signal.notify();
        Ok(())
      }
      Err(err @ QueueError::Disconnected) | Err(err @ QueueError::Closed(_)) => {
        self.closed.set(true);
        Err(err)
      }
      Err(err) => Err(err),
    }
  }

  /// メッセージを非同期的に送信します。
  ///
  /// 現在の実装では `try_send` を呼び出すだけですが、
  /// 将来的にバックプレッシャー対応などの拡張が可能です。
  ///
  /// # Arguments
  /// - `message`: 送信するメッセージ
  ///
  /// # Returns
  /// 成功時は `Ok(())`、失敗時は `Err(QueueError)`
  pub async fn send<M>(&self, message: M) -> Result<(), QueueError<M>>
  where
    Q: QueueRw<M>,
    S: MailboxSignal,
    M: Element, {
    self.try_send(message)
  }
}

impl<M, Q, S> Mailbox<M> for QueueMailbox<Q, S>
where
  Q: QueueRw<M>,
  S: MailboxSignal,
  M: Element,
{
  type RecvFuture<'a>
    = QueueMailboxRecv<'a, Q, S, M>
  where
    Self: 'a;
  type SendError = QueueError<M>;

  fn try_send(&self, message: M) -> Result<(), Self::SendError> {
    match self.queue.offer(message) {
      Ok(()) => {
        self.signal.notify();
        Ok(())
      }
      Err(err @ QueueError::Disconnected) | Err(err @ QueueError::Closed(_)) => {
        self.closed.set(true);
        Err(err)
      }
      Err(err) => Err(err),
    }
  }

  fn recv(&self) -> Self::RecvFuture<'_> {
    QueueMailboxRecv {
      mailbox: self,
      wait: None,
      marker: PhantomData,
    }
  }

  fn len(&self) -> QueueSize {
    self.queue.len()
  }

  fn capacity(&self) -> QueueSize {
    self.queue.capacity()
  }

  fn close(&self) {
    self.queue.clean_up();
    self.signal.notify();
    self.closed.set(true);
  }

  fn is_closed(&self) -> bool {
    self.closed.get()
  }
}

/// メッセージ受信用のFuture。
///
/// メールボックスからメッセージを非同期的に受信するためのFuture実装です。
/// メッセージが到着するまで待機し、到着したメッセージを返します。
///
/// # 型パラメータ
/// - `'a`: メールボックスへの参照のライフタイム
/// - `Q`: メッセージキューの実装型
/// - `S`: 通知シグナルの実装型
/// - `M`: 受信するメッセージの型
pub struct QueueMailboxRecv<'a, Q, S, M>
where
  Q: QueueRw<M>,
  S: MailboxSignal,
  M: Element, {
  mailbox: &'a QueueMailbox<Q, S>,
  wait: Option<S::WaitFuture<'a>>,
  marker: PhantomData<M>,
}

impl<'a, Q, S, M> Future for QueueMailboxRecv<'a, Q, S, M>
where
  Q: QueueRw<M>,
  S: MailboxSignal,
  M: Element,
{
  type Output = Result<M, QueueError<M>>;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = unsafe { self.get_unchecked_mut() };
    if this.mailbox.closed.get() {
      return Poll::Ready(Err(QueueError::Disconnected));
    }
    loop {
      match this.mailbox.queue.poll() {
        Ok(Some(message)) => {
          this.wait = None;
          return Poll::Ready(Ok(message));
        }
        Ok(None) => {
          if this.wait.is_none() {
            this.wait = Some(this.mailbox.signal.wait());
          }
        }
        Err(QueueError::Disconnected) => {
          this.mailbox.closed.set(true);
          this.wait = None;
          return Poll::Ready(Err(QueueError::Disconnected));
        }
        Err(QueueError::Closed(message)) => {
          this.mailbox.closed.set(true);
          this.wait = None;
          return Poll::Ready(Ok(message));
        }
        Err(QueueError::Full(_)) | Err(QueueError::OfferError(_)) => {
          return Poll::Pending;
        }
      }

      if let Some(wait) = this.wait.as_mut() {
        match unsafe { Pin::new_unchecked(wait) }.poll(cx) {
          Poll::Ready(()) => {
            this.wait = None;
            continue;
          }
          Poll::Pending => return Poll::Pending,
        }
      }
    }
  }
}
