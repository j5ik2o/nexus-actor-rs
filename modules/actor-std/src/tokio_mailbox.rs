use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use nexus_actor_core_rs::{Mailbox, MailboxSignal, QueueMailbox, QueueMailboxProducer, QueueMailboxRecv};
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

#[derive(Clone, Debug)]
struct NotifySignal {
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

#[derive(Clone, Debug)]
pub enum TokioQueue<M>
where
  M: Element, {
  Unbounded(ArcMpscUnboundedQueue<M>),
  Bounded(ArcMpscBoundedQueue<M>),
}

impl<M> TokioQueue<M>
where
  M: Element,
{
  fn with_capacity(capacity: usize) -> Self {
    if capacity == 0 {
      Self::Unbounded(ArcMpscUnboundedQueue::new())
    } else {
      Self::Bounded(ArcMpscBoundedQueue::new(capacity))
    }
  }
}

impl<M> QueueBase<M> for TokioQueue<M>
where
  M: Element,
{
  fn len(&self) -> QueueSize {
    match self {
      Self::Unbounded(queue) => queue.len(),
      Self::Bounded(queue) => queue.len(),
    }
  }

  fn capacity(&self) -> QueueSize {
    match self {
      Self::Unbounded(queue) => queue.capacity(),
      Self::Bounded(queue) => queue.capacity(),
    }
  }
}

impl<M> QueueRw<M> for TokioQueue<M>
where
  M: Element,
{
  fn offer(&self, element: M) -> Result<(), QueueError<M>> {
    match self {
      Self::Unbounded(queue) => queue.offer(element),
      Self::Bounded(queue) => queue.offer(element),
    }
  }

  fn poll(&self) -> Result<Option<M>, QueueError<M>> {
    match self {
      Self::Unbounded(queue) => queue.poll(),
      Self::Bounded(queue) => queue.poll(),
    }
  }

  fn clean_up(&self) {
    match self {
      Self::Unbounded(queue) => queue.clean_up(),
      Self::Bounded(queue) => queue.clean_up(),
    }
  }
}

impl<M> TokioMailbox<M>
where
  M: Element,
  TokioQueue<M>: Clone,
{
  pub fn new(capacity: usize) -> (Self, TokioMailboxSender<M>) {
    let queue = TokioQueue::with_capacity(capacity);
    let signal = NotifySignal::default();
    Self::with_parts(queue, signal)
  }

  pub fn unbounded() -> (Self, TokioMailboxSender<M>) {
    Self::new(0)
  }

  pub fn producer(&self) -> TokioMailboxSender<M> {
    TokioMailboxSender {
      inner: self.inner.producer(),
    }
  }

  fn with_parts(queue: TokioQueue<M>, signal: NotifySignal) -> (Self, TokioMailboxSender<M>) {
    let mailbox = QueueMailbox::new(queue, signal);
    let sender = TokioMailboxSender {
      inner: mailbox.producer(),
    };
    (Self { inner: mailbox }, sender)
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
    self.try_send(message)
  }
}

pub struct TokioMailboxRecvFuture<'a, M>
where
  M: Element,
  TokioQueue<M>: Clone, {
  inner: QueueMailboxRecv<'a, TokioQueue<M>, NotifySignal, M>,
}

impl<'a, M> Future for TokioMailboxRecvFuture<'a, M>
where
  M: Element,
  TokioQueue<M>: Clone,
{
  type Output = M;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    unsafe { self.map_unchecked_mut(|this| &mut this.inner) }.poll(cx)
  }
}

impl<M> Mailbox<M> for TokioMailbox<M>
where
  M: Element,
  TokioQueue<M>: Clone,
{
  type RecvFuture<'a>
    = TokioMailboxRecvFuture<'a, M>
  where
    Self: 'a;
  type SendError = QueueError<M>;

  fn try_send(&self, message: M) -> Result<(), Self::SendError> {
    self.inner.try_send(message)
  }

  fn recv(&self) -> Self::RecvFuture<'_> {
    TokioMailboxRecvFuture {
      inner: self.inner.recv(),
    }
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
