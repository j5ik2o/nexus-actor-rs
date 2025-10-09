use core::future::Future;
use core::time::Duration;

use crate::FailureEventStream;

/// Dependency collection required for `ActorSystem` initialization.
///
/// Holds all components necessary for constructing the actor system.
pub struct ActorSystemParts<MF, S, T, E> {
  /// Mailbox factory
  pub mailbox_factory: MF,
  /// Task spawner
  pub spawner: S,
  /// Timer
  pub timer: T,
  /// Event stream
  pub event_stream: E,
}

impl<MF, S, T, E> ActorSystemParts<MF, S, T, E> {
  /// Creates a new `ActorSystemParts`.
  ///
  /// # Arguments
  ///
  /// * `mailbox_factory` - Mailbox factory
  /// * `spawner` - Task spawner
  /// * `timer` - Timer
  /// * `event_stream` - Event stream
  pub fn new(mailbox_factory: MF, spawner: S, timer: T, event_stream: E) -> Self {
    Self {
      mailbox_factory,
      spawner,
      timer,
      event_stream,
    }
  }
}

/// Peripheral components that the driver continues to hold after `ActorSystem` construction.
///
/// Set of handles required for runtime operations, excluding the mailbox factory.
pub struct ActorSystemHandles<S, T, E> {
  /// Task spawner
  pub spawner: S,
  /// Timer
  pub timer: T,
  /// Event stream
  pub event_stream: E,
}

impl<MF, S, T, E> ActorSystemParts<MF, S, T, E>
where
  E: FailureEventStream,
{
  /// Splits `ActorSystemParts` into mailbox factory and handles.
  ///
  /// # Returns
  ///
  /// Tuple of mailbox factory and `ActorSystemHandles`
  pub fn split(self) -> (MF, ActorSystemHandles<S, T, E>) {
    let Self {
      mailbox_factory,
      spawner,
      timer,
      event_stream,
    } = self;

    let handles = ActorSystemHandles {
      spawner,
      timer,
      event_stream,
    };

    (mailbox_factory, handles)
  }
}

/// Interface for abstracting asynchronous task execution.
///
/// Provides an abstraction layer for spawning asynchronous tasks in an environment-independent manner.
pub trait Spawn {
  /// Spawns a new asynchronous task.
  ///
  /// # Arguments
  ///
  /// * `fut` - Asynchronous task to execute
  fn spawn(&self, fut: impl Future<Output = ()> + Send + 'static);
}

/// Generic timer abstraction.
///
/// Provides an abstraction layer for delayed execution in an environment-independent manner.
pub trait Timer {
  /// Future type for sleep operation
  type SleepFuture<'a>: Future<Output = ()> + 'a
  where
    Self: 'a;

  /// Returns a Future that sleeps for the specified duration.
  ///
  /// # Arguments
  ///
  /// * `duration` - Duration to sleep
  fn sleep(&self, duration: Duration) -> Self::SleepFuture<'_>;
}
