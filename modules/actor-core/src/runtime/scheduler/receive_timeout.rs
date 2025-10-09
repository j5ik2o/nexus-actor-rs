use alloc::boxed::Box;
use core::time::Duration;

use nexus_utils_core_rs::Element;

use crate::MapSystemShared;
use crate::{MailboxFactory, PriorityEnvelope, QueueMailboxProducer};

/// Scheduler abstraction for managing actor `ReceiveTimeout`.
///
/// Provides a unified interface for setting/resetting/stopping timeouts,
/// so that `actor-core` doesn't need to directly handle runtime-dependent timers.
/// By calling `notify_activity` after user message processing,
/// the runtime side can re-arm with any implementation (tokio / embedded software timer, etc.).
pub trait ReceiveTimeoutScheduler: Send {
  /// Sets/re-arms the timer with the specified duration.
  fn set(&mut self, duration: Duration);

  /// Stops the timer.
  fn cancel(&mut self);

  /// Notifies of activity (like user messages) that should reset the timeout.
  fn notify_activity(&mut self);
}

/// Factory for creating schedulers.
///
/// Receives a priority mailbox and SystemMessage conversion function when creating actors,
/// and assembles a runtime-specific `ReceiveTimeoutScheduler`.
/// By registering via `ActorSystem::set_receive_timeout_scheduler_factory`,
/// all actors can handle timeouts with the same policy.
pub trait ReceiveTimeoutSchedulerFactory<M, R>: Send + Sync
where
  M: Element + 'static,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  /// Creates an actor-specific scheduler by receiving a priority mailbox and SystemMessage conversion function.
  fn create(
    &self,
    sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
    map_system: MapSystemShared<M>,
  ) -> Box<dyn ReceiveTimeoutScheduler>;
}
