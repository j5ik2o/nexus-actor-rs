use core::future::Future;

use nexus_utils_core_rs::{Element, QueueError, QueueRw, QueueSize};

use super::queue_mailbox::MailboxOptions;

pub type MailboxPair<Q, S> = (super::QueueMailbox<Q, S>, super::QueueMailboxProducer<Q, S>);

/// Mailbox abstraction that decouples message queue implementations from core logic.
pub trait Mailbox<M> {
  type SendError;
  type RecvFuture<'a>: Future<Output = Result<M, QueueError<M>>> + 'a
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

  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
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

pub trait MailboxRuntime {
  type Signal: MailboxSignal;

  type Queue<M>: QueueRw<M> + Clone
  where
    M: Element;

  fn build_mailbox<M>(&self, options: MailboxOptions) -> MailboxPair<Self::Queue<M>, Self::Signal>
  where
    M: Element;

  fn build_default_mailbox<M>(&self) -> MailboxPair<Self::Queue<M>, Self::Signal>
  where
    M: Element, {
    self.build_mailbox(MailboxOptions::default())
  }
}
