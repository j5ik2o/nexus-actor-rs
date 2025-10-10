use core::future::Future;

use nexus_utils_core_rs::{Element, QueueError, QueueRw, QueueSize};

use crate::runtime::message::MetadataStorageMode;

use super::queue_mailbox::MailboxOptions;

/// Type alias for mailbox and producer pair.
///
/// Pair of receiver and sender handles returned when creating a mailbox.
pub type MailboxPair<Q, S> = (super::QueueMailbox<Q, S>, super::QueueMailboxProducer<Q, S>);

/// Mailbox abstraction that decouples message queue implementations from core logic.
///
/// Abstraction trait that decouples message queue implementations from core logic.
/// Enables unified handling of various queue implementations (bounded/unbounded, prioritized, etc.).
///
/// # Type Parameters
/// - `M`: Type of the message to process
pub trait Mailbox<M> {
  /// Error type for message sending
  type SendError;

  /// Future type for message reception
  type RecvFuture<'a>: Future<Output = Result<M, QueueError<M>>> + 'a
  where
    Self: 'a;

  /// Attempts to send a message (non-blocking).
  ///
  /// # Arguments
  /// - `message`: Message to send
  ///
  /// # Returns
  /// `Ok(())` on success, `Err(SendError)` on failure
  fn try_send(&self, message: M) -> Result<(), Self::SendError>;

  /// Receives a message asynchronously.
  ///
  /// # Returns
  /// Future for message reception
  fn recv(&self) -> Self::RecvFuture<'_>;

  /// Gets the number of messages in the mailbox.
  ///
  /// Default implementation returns unlimited.
  fn len(&self) -> QueueSize {
    QueueSize::limitless()
  }

  /// Gets the capacity of the mailbox.
  ///
  /// Default implementation returns unlimited.
  fn capacity(&self) -> QueueSize {
    QueueSize::limitless()
  }

  /// Checks if the mailbox is empty.
  ///
  /// # Returns
  /// `true` if empty, `false` if there are messages
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }

  /// Closes the mailbox.
  ///
  /// Default implementation does nothing.
  fn close(&self) {}

  /// Checks if the mailbox is closed.
  ///
  /// Default implementation always returns `false`.
  ///
  /// # Returns
  /// `true` if closed, `false` if open
  fn is_closed(&self) -> bool {
    false
  }
}

/// Notification primitive used by `QueueMailbox` to park awaiting receivers until
/// new messages are available.
///
/// Synchronization primitive used for notifying message arrivals.
/// Provides a mechanism for receivers to wait for messages and senders to notify arrivals.
pub trait MailboxSignal: Clone {
  /// Future type for waiting
  type WaitFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;

  /// Notifies waiting receivers that a message has arrived.
  fn notify(&self);

  /// Waits for a message arrival.
  ///
  /// # Returns
  /// Future that waits for notification
  fn wait(&self) -> Self::WaitFuture<'_>;
}

/// Marker trait describing the synchronization requirements for a mailbox factory.
pub trait MailboxConcurrency: Copy + 'static {}

/// Thread-safe mailbox mode requiring `Send + Sync` types.
#[derive(Debug, Clone, Copy, Default)]
pub struct ThreadSafe;

impl MailboxConcurrency for ThreadSafe {}

/// Single-threaded mailbox mode without additional synchronization requirements.
#[derive(Debug, Clone, Copy, Default)]
pub struct SingleThread;

impl MailboxConcurrency for SingleThread {}

/// Factory trait for creating mailboxes.
///
/// Generates mailbox and queue implementations according to
/// specific async runtimes (Tokio, Async-std, etc.).
pub trait MailboxFactory {
  /// Declares the concurrency mode for this factory.
  type Concurrency: MailboxConcurrency + MetadataStorageMode;

  /// Type of notification signal
  type Signal: MailboxSignal;

  /// Type of message queue
  type Queue<M>: QueueRw<M> + Clone
  where
    M: Element;

  /// Creates a mailbox with the specified options.
  ///
  /// # Arguments
  /// - `options`: Capacity settings for the mailbox
  ///
  /// # Returns
  /// Pair of `(mailbox, producer)`
  fn build_mailbox<M>(&self, options: MailboxOptions) -> MailboxPair<Self::Queue<M>, Self::Signal>
  where
    M: Element;

  /// Creates a mailbox with default settings.
  ///
  /// # Returns
  /// Pair of `(mailbox, producer)`
  fn build_default_mailbox<M>(&self) -> MailboxPair<Self::Queue<M>, Self::Signal>
  where
    M: Element, {
    self.build_mailbox(MailboxOptions::default())
  }
}
