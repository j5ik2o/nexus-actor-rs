use std::sync::Arc;

use nexus_actor_core_rs::{
  Mailbox, MailboxFactory, MailboxOptions, MailboxPair, MailboxSignal, QueueMailbox, QueueMailboxProducer,
  QueueMailboxRecv,
};
use nexus_utils_std_rs::{ArcMpscBoundedQueue, ArcMpscUnboundedQueue};
use nexus_utils_std_rs::{Element, QueueBase, QueueError, QueueRw, QueueSize};
use tokio::sync::{futures::Notified, Notify};

/// Mailbox implementation for Tokio runtime
///
/// An asynchronous queue that manages message delivery to actors.
#[derive(Clone, Debug)]
pub struct TokioMailbox<M>
where
  M: Element,
{
  inner: QueueMailbox<TokioQueue<M>, NotifySignal>,
}

/// Sender handle for Tokio mailbox
///
/// Provides an interface specialized for sending messages.
#[derive(Clone, Debug)]
pub struct TokioMailboxSender<M>
where
  M: Element,
{
  inner: QueueMailboxProducer<TokioQueue<M>, NotifySignal>,
}

/// Factory that creates Tokio mailboxes
///
/// Creates bounded and unbounded mailboxes.
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
  M: Element,
{
  inner: Arc<TokioQueueKind<M>>,
}

#[derive(Debug)]
enum TokioQueueKind<M>
where
  M: Element,
{
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
  /// Creates a mailbox with the specified options
  ///
  /// # Arguments
  /// * `options` - Configuration options for the mailbox
  ///
  /// # Returns
  /// A pair of mailbox and sender handle
  pub fn mailbox<M>(&self, options: MailboxOptions) -> (TokioMailbox<M>, TokioMailboxSender<M>)
  where
    M: Element,
  {
    let (mailbox, sender) = self.build_mailbox::<M>(options);
    (TokioMailbox { inner: mailbox }, TokioMailboxSender { inner: sender })
  }

  /// Creates a bounded mailbox with the specified capacity
  ///
  /// # Arguments
  /// * `capacity` - Maximum capacity of the mailbox
  ///
  /// # Returns
  /// A pair of mailbox and sender handle
  pub fn with_capacity<M>(&self, capacity: usize) -> (TokioMailbox<M>, TokioMailboxSender<M>)
  where
    M: Element,
  {
    self.mailbox(MailboxOptions::with_capacity(capacity))
  }

  /// Creates an unbounded mailbox
  ///
  /// # Returns
  /// A pair of mailbox and sender handle
  pub fn unbounded<M>(&self) -> (TokioMailbox<M>, TokioMailboxSender<M>)
  where
    M: Element,
  {
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
    M: Element,
  {
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
  /// Creates a mailbox with the specified capacity
  ///
  /// # Arguments
  /// * `capacity` - Maximum capacity of the mailbox
  ///
  /// # Returns
  /// A pair of mailbox and sender handle
  pub fn new(capacity: usize) -> (Self, TokioMailboxSender<M>) {
    TokioMailboxFactory.with_capacity(capacity)
  }

  /// Creates an unbounded mailbox
  ///
  /// # Returns
  /// A pair of mailbox and sender handle
  pub fn unbounded() -> (Self, TokioMailboxSender<M>) {
    TokioMailboxFactory.unbounded()
  }

  /// Creates a new sender handle
  ///
  /// # Returns
  /// A `TokioMailboxSender` for sending messages
  pub fn producer(&self) -> TokioMailboxSender<M>
  where
    TokioQueue<M>: Clone,
    NotifySignal: Clone,
  {
    TokioMailboxSender {
      inner: self.inner.producer(),
    }
  }

  /// Returns a reference to the internal queue mailbox
  ///
  /// # Returns
  /// An immutable reference to the internal mailbox
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
  /// Attempts to send a message (non-blocking)
  ///
  /// # Arguments
  /// * `message` - The message to send
  ///
  /// # Returns
  /// `Ok(())` on success, or an error with the message on failure
  ///
  /// # Errors
  /// Returns `QueueError::Full` if the queue is full
  pub fn try_send(&self, message: M) -> Result<(), QueueError<M>> {
    self.inner.try_send(message)
  }

  /// Sends a message asynchronously
  ///
  /// # Arguments
  /// * `message` - The message to send
  ///
  /// # Returns
  /// `Ok(())` on success, or an error with the message on failure
  ///
  /// # Errors
  /// Returns `QueueError::Closed` if the mailbox is closed
  pub async fn send(&self, message: M) -> Result<(), QueueError<M>> {
    self.inner.send(message).await
  }

  /// Returns a reference to the internal queue mailbox producer
  ///
  /// # Returns
  /// An immutable reference to the internal producer
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
