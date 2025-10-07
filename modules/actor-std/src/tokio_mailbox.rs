use std::sync::Arc;

use nexus_actor_core_rs::{
  Mailbox, MailboxOptions, MailboxPair, MailboxRuntime, MailboxSignal, QueueMailbox, QueueMailboxProducer,
  QueueMailboxRecv,
};
use nexus_utils_std_rs::collections::queue::mpsc::{ArcMpscBoundedQueue, ArcMpscUnboundedQueue};
use nexus_utils_std_rs::{Element, QueueBase, QueueError, QueueRw, QueueSize};
use tokio::sync::{futures::Notified, Notify};

#[derive(Clone, Debug)]
pub struct TokioMailbox<M>
where
  M: Element, {
  inner: QueueMailbox<TokioQueue<M>, NotifySignal>,
}

#[derive(Clone, Debug)]
pub struct TokioMailboxSender<M>
where
  M: Element, {
  inner: QueueMailboxProducer<TokioQueue<M>, NotifySignal>,
}

#[derive(Clone, Debug, Default)]
pub struct TokioMailboxRuntime;

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

impl TokioMailboxRuntime {
  pub fn mailbox<M>(&self, options: MailboxOptions) -> (TokioMailbox<M>, TokioMailboxSender<M>)
  where
    M: Element, {
    let (mailbox, sender) = self.build_mailbox::<M>(options);
    (TokioMailbox { inner: mailbox }, TokioMailboxSender { inner: sender })
  }

  pub fn with_capacity<M>(&self, capacity: usize) -> (TokioMailbox<M>, TokioMailboxSender<M>)
  where
    M: Element, {
    self.mailbox(MailboxOptions::with_capacity(capacity))
  }

  pub fn unbounded<M>(&self) -> (TokioMailbox<M>, TokioMailboxSender<M>)
  where
    M: Element, {
    self.mailbox(MailboxOptions::unbounded())
  }
}

impl MailboxRuntime for TokioMailboxRuntime {
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
  pub fn new(capacity: usize) -> (Self, TokioMailboxSender<M>) {
    TokioMailboxRuntime::default().with_capacity(capacity)
  }

  pub fn unbounded() -> (Self, TokioMailboxSender<M>) {
    TokioMailboxRuntime::default().unbounded()
  }

  pub fn producer(&self) -> TokioMailboxSender<M>
  where
    TokioQueue<M>: Clone,
    NotifySignal: Clone, {
    TokioMailboxSender {
      inner: self.inner.producer(),
    }
  }

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
  pub fn try_send(&self, message: M) -> Result<(), QueueError<M>> {
    self.inner.try_send(message)
  }

  pub async fn send(&self, message: M) -> Result<(), QueueError<M>> {
    self.inner.send(message).await
  }

  pub fn inner(&self) -> &QueueMailboxProducer<TokioQueue<M>, NotifySignal> {
    &self.inner
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use nexus_utils_std_rs::QueueError;

  #[tokio::test(flavor = "current_thread")]
  async fn runtime_with_capacity_enforces_bounds() {
    let runtime = TokioMailboxRuntime::default();
    let (mailbox, sender) = runtime.with_capacity::<u32>(2);

    sender.try_send(1).expect("first message accepted");
    sender.try_send(2).expect("second message accepted");
    assert!(matches!(sender.try_send(3), Err(QueueError::Full(3))));
    assert_eq!(mailbox.len().to_usize(), 2);

    let first = mailbox.recv().await;
    let second = mailbox.recv().await;

    assert_eq!(first, 1);
    assert_eq!(second, 2);
    assert_eq!(mailbox.len().to_usize(), 0);
  }

  #[tokio::test(flavor = "current_thread")]
  async fn runtime_unbounded_mailbox_accepts_multiple_messages() {
    let runtime = TokioMailboxRuntime::default();
    let (mailbox, sender) = runtime.unbounded::<u32>();

    for value in 0..32_u32 {
      sender.send(value).await.expect("send succeeds");
    }

    assert!(mailbox.capacity().is_limitless());

    for expected in 0..32_u32 {
      let received = mailbox.recv().await;
      assert_eq!(received, expected);
    }

    assert_eq!(mailbox.len().to_usize(), 0);
  }
}
