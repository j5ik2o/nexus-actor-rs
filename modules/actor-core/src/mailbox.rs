use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::task::{Context, Poll};

use nexus_utils_core_rs::{Element, QueueError, QueueRw, QueueSize};

/// Mailbox abstraction that decouples message queue implementations from core logic.
pub trait Mailbox<M> {
  type SendError;
  type RecvFuture<'a>: Future<Output = M> + 'a
  where
    Self: 'a;

  fn try_send(&self, message: M) -> Result<(), Self::SendError>;
  fn recv(&self) -> Self::RecvFuture<'_>;

  fn len(&self) -> QueueSize {
    QueueSize::limitless()
  }

  fn capacity(&self) -> QueueSize {
    QueueSize::limitless()
  }

  fn close(&self) {}

  fn is_closed(&self) -> bool {
    false
  }
}

/// Notification primitive used by [`QueueMailbox`] to park awaiting receivers until
/// new messages are available.
pub trait MailboxSignal: Clone {
  type WaitFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;

  fn notify(&self);
  fn wait(&self) -> Self::WaitFuture<'_>;
}

/// Mailbox implementation backed by a generic queue and notification signal.
#[derive(Debug)]
pub struct QueueMailbox<Q, S> {
  queue: Q,
  signal: S,
}

impl<Q, S> QueueMailbox<Q, S> {
  pub const fn new(queue: Q, signal: S) -> Self {
    Self { queue, signal }
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
    }
  }
}

/// Sending handle that shares queue ownership with [`QueueMailbox`].
#[derive(Clone, Debug)]
pub struct QueueMailboxProducer<Q, S> {
  queue: Q,
  signal: S,
}

impl<Q, S> QueueMailboxProducer<Q, S> {
  pub fn try_send<M>(&self, message: M) -> Result<(), QueueError<M>>
  where
    Q: QueueRw<M>,
    S: MailboxSignal,
    M: Element, {
    self.queue.offer(message).map(|_| {
      self.signal.notify();
    })
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
    self.queue.offer(message).map(|_| {
      self.signal.notify();
    })
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
  }

  fn is_closed(&self) -> bool {
    false
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
  type Output = M;

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    let this = unsafe { self.get_unchecked_mut() };
    loop {
      match this.mailbox.queue.poll() {
        Ok(Some(message)) => {
          this.wait = None;
          return Poll::Ready(message);
        }
        Ok(None) => {
          if this.wait.is_none() {
            this.wait = Some(this.mailbox.signal.wait());
          }
        }
        Err(QueueError::Disconnected) => panic!("mailbox disconnected"),
        Err(QueueError::Closed(_)) => panic!("mailbox closed"),
        Err(QueueError::Full(_)) | Err(QueueError::OfferError(_)) => {
          unreachable!("send-side errors should not occur during recv")
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
