use core::fmt;

use crate::ActorId;
use crate::MailboxFactory;
use crate::SupervisorDirective;
use nexus_utils_core_rs::Element;

/// Supervisor strategy. Corresponds to protoactor-go's Strategy.
///
/// Trait that defines the strategy applied when an actor fails.
/// Determines how the parent actor (guardian) handles child actor failures.
///
/// # Type Parameters
/// - `M`: Message type processed by the mailbox
/// - `R`: Factory type that generates mailboxes
pub trait GuardianStrategy<M, R>: Send + 'static
where
  M: Element,
  R: MailboxFactory, {
  /// Determines the handling policy when an actor fails.
  ///
  /// # Arguments
  /// - `actor`: ID of the failed actor
  /// - `error`: Detailed information about the error that occurred
  ///
  /// # Returns
  /// Supervisor directive (Restart, Stop, Resume, Escalate, etc.)
  fn decide(&mut self, actor: ActorId, error: &dyn fmt::Debug) -> SupervisorDirective;

  /// Hook called before actor startup.
  ///
  /// Default implementation does nothing. Override if needed.
  ///
  /// # Arguments
  /// - `_actor`: ID of the actor being started
  fn before_start(&mut self, _actor: ActorId) {}

  /// Hook called after actor restart.
  ///
  /// Default implementation does nothing. Override if needed.
  ///
  /// # Arguments
  /// - `_actor`: ID of the restarted actor
  fn after_restart(&mut self, _actor: ActorId) {}
}

/// Simplest strategy: Always instruct Restart.
///
/// Supervisor strategy that always instructs actor restart regardless of error type.
/// Suitable when expecting automatic recovery from temporary failures.
///
/// # Example Usage
/// ```ignore
/// let strategy = AlwaysRestart;
/// // A guardian using this strategy will always attempt to restart
/// // child actors when they fail
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct AlwaysRestart;

impl<M, R> GuardianStrategy<M, R> for AlwaysRestart
where
  M: Element,
  R: MailboxFactory,
{
  fn decide(&mut self, _actor: ActorId, _error: &dyn fmt::Debug) -> SupervisorDirective {
    SupervisorDirective::Restart
  }
}
