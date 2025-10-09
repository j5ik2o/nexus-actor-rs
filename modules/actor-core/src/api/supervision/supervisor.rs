use core::fmt;

/// Action returned by the supervisor.
///
/// Instructs how the supervisor should respond when an actor fails.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SupervisorDirective {
  /// Stop the actor.
  Stop,
  /// Ignore the error and continue processing.
  Resume,
  /// Restart the actor.
  Restart,
  /// Escalate to parent.
  Escalate,
}

/// Base supervisor trait.
///
/// Defines actor failure handling strategy and controls behavior on failure.
pub trait Supervisor<M>: Send + 'static {
  /// Hook called before failure handling.
  ///
  /// Default implementation does nothing.
  fn before_handle(&mut self) {}

  /// Hook called after failure handling.
  ///
  /// Default implementation does nothing.
  fn after_handle(&mut self) {}

  /// Determines the handling policy for failures.
  ///
  /// # Arguments
  ///
  /// * `_error` - Information about the error that occurred
  ///
  /// # Returns
  ///
  /// `SupervisorDirective` to execute
  fn decide(&mut self, _error: &dyn fmt::Debug) -> SupervisorDirective {
    SupervisorDirective::Stop
  }
}

/// No-op supervisor implementation.
///
/// Returns `Resume` for all failures and continues processing.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopSupervisor;

impl<M> Supervisor<M> for NoopSupervisor {
  /// Returns `Resume` for all failures.
  fn decide(&mut self, _error: &dyn fmt::Debug) -> SupervisorDirective {
    SupervisorDirective::Resume
  }
}
