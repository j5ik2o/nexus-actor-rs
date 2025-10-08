use alloc::sync::Arc;
use core::marker::PhantomData;

use crate::failure::{FailureEvent, FailureInfo};
use crate::{MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::Element;

use super::EscalationSink;

pub type FailureEventHandler = Arc<dyn Fn(&FailureInfo) + Send + Sync>;
pub type FailureEventListener = Arc<dyn Fn(FailureEvent) + Send + Sync>;

pub struct RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
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
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new() -> Self {
    Self {
      event_handler: None,
      _marker: PhantomData,
      event_listener: None,
    }
  }

  pub fn set_event_handler(&mut self, handler: Option<FailureEventHandler>) {
    self.event_handler = handler;
  }

  pub fn set_event_listener(&mut self, listener: Option<FailureEventListener>) {
    self.event_listener = listener;
  }
}

impl<M, R> Default for RootEscalationSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
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
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn handle(&mut self, info: FailureInfo, _already_handled: bool) -> Result<(), FailureInfo> {
    #[cfg(feature = "std")]
    {
      use tracing::error;
      error!(actor = ?info.actor, reason = %info.reason, path = ?info.path.segments(), "actor escalation reached root guardian");
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
