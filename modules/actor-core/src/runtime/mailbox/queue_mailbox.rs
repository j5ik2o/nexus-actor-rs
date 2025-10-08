use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll};

use nexus_utils_core_rs::Flag;
use nexus_utils_core_rs::{Element, QueueError, QueueRw, QueueSize};

use super::traits::{Mailbox, MailboxSignal};

/// Runtime-agnostic construction options for [`QueueMailbox`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxOptions {
  pub capacity: QueueSize,
  pub priority_capacity: QueueSize,
}

impl MailboxOptions {
  pub const fn with_capacity(capacity: usize) -> Self {
    Self {
      capacity: QueueSize::limited(capacity),
      priority_capacity: QueueSize::limitless(),
    }
  }

  pub const fn with_capacities(capacity: QueueSize, priority_capacity: QueueSize) -> Self {
    Self {
      capacity,
      priority_capacity,
    }
  }

  pub const fn with_priority_capacity(mut self, priority_capacity: QueueSize) -> Self {
    self.priority_capacity = priority_capacity;
    self
  }

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
#[derive(Debug)]
pub struct QueueMailbox<Q, S> {
  queue: Q,
  signal: S,
  closed: Flag,
}

impl<Q, S> QueueMailbox<Q, S> {
  pub fn new(queue: Q, signal: S) -> Self {
    Self {
      queue,
      signal,
      closed: Flag::default(),
    }
  }

  pub fn queue(&self) -> &Q {
    &self.queue
  }

  pub fn signal(&self) -> &S {
    &self.signal
  }

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
