use alloc::sync::Arc;
use core::marker::PhantomData;

use nexus_utils_core_rs::Element;

use super::failure::FailureEvent;
use crate::{FailureInfo, MailboxFactory, PriorityEnvelope};

/// Handler for notifying failure events externally.
///
/// Receives actor failure information and performs tasks like logging or notifications to monitoring systems.
pub type FailureEventHandler = Arc<dyn Fn(&FailureInfo) + Send + Sync>;

/// Listener for receiving failure events as a stream.
///
/// Subscribes to failure events from the entire actor system and executes custom processing.
pub type FailureEventListener = Arc<dyn Fn(FailureEvent) + Send + Sync>;

/// Sink for controlling how `FailureInfo` is propagated upward.
///
/// Defines escalation processing for actor failure information.
pub trait EscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  /// Processes failure information.
  ///
  /// # Arguments
  ///
  /// * `info` - Failure information
  /// * `already_handled` - If `true`, indicates that processing has already been completed locally
  ///
  /// # Returns
  ///
  /// `Ok(())` on success, `Err(FailureInfo)` if processing failed
  fn handle(&mut self, info: FailureInfo, already_handled: bool) -> Result<(), FailureInfo>;
}

/// `EscalationSink` implementation for root guardian.
///
/// Handles failures at the root level of the actor system.
/// Ultimately processes failures that cannot be escalated further.
pub struct RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  event_handler: Option<FailureEventHandler>,
  event_listener: Option<FailureEventListener>,
  _marker: PhantomData<(M, R)>,
}

impl<M, R> RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  /// Creates a new `RootEscalationSink`.
  ///
  /// By default, no handler or listener is configured.
  pub fn new() -> Self {
    Self {
      event_handler: None,
      event_listener: None,
      _marker: PhantomData,
    }
  }

  /// Sets the failure event handler.
  ///
  /// # Arguments
  ///
  /// * `handler` - Failure event handler, or `None`
  pub fn set_event_handler(&mut self, handler: Option<FailureEventHandler>) {
    self.event_handler = handler;
  }

  /// Sets the failure event listener.
  ///
  /// # Arguments
  ///
  /// * `listener` - Failure event listener, or `None`
  pub fn set_event_listener(&mut self, listener: Option<FailureEventListener>) {
    self.event_listener = listener;
  }
}

impl<M, R> Default for RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<M, R> EscalationSink<M, R> for RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  /// Processes failure information at root level.
  ///
  /// Performs log output, handler invocation, and listener notification.
  ///
  /// # Arguments
  ///
  /// * `info` - Failure information
  /// * `_already_handled` - Unused (always executes processing at root level)
  ///
  /// # Returns
  ///
  /// Always returns `Ok(())`
  fn handle(&mut self, info: FailureInfo, _already_handled: bool) -> Result<(), FailureInfo> {
    #[cfg(feature = "std")]
    {
      use tracing::error;
      error!(
        actor = ?info.actor,
        reason = %info.reason,
        path = ?info.path.segments(),
        "actor escalation reached root guardian"
      );
    }

    if let Some(handler) = self.event_handler.as_ref() {
      handler(&info);
    }

    if let Some(listener) = self.event_listener.as_ref() {
      listener(FailureEvent::RootEscalated(info.clone()));
    }

    Ok(())
  }
}
