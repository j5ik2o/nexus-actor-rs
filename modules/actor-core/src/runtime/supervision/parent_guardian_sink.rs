use alloc::sync::Arc;

use crate::runtime::context::{InternalActorRef, MapSystemFn};
use crate::EscalationSink;
use crate::FailureInfo;
use crate::MailboxFactory;
use crate::{PriorityEnvelope, SystemMessage};
use nexus_utils_core_rs::Element;

/// Sink that forwards `SystemMessage::Escalate` to parent Guardian.
pub(crate) struct ParentGuardianSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  control_ref: InternalActorRef<M, R>,
  map_system: Arc<MapSystemFn<M>>,
}

impl<M, R> ParentGuardianSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub(crate) fn new(control_ref: InternalActorRef<M, R>, map_system: Arc<MapSystemFn<M>>) -> Self {
    Self {
      control_ref,
      map_system,
    }
  }
}

impl<M, R> EscalationSink<M, R> for ParentGuardianSink<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn handle(&mut self, info: FailureInfo, already_handled: bool) -> Result<(), FailureInfo> {
    if already_handled {
      return Ok(());
    }

    if let Some(parent_info) = info.escalate_to_parent() {
      let envelope =
        PriorityEnvelope::from_system(SystemMessage::Escalate(parent_info.clone())).map(|sys| (self.map_system)(sys));
      if self.control_ref.sender().try_send(envelope).is_ok() {
        return Ok(());
      }
    } else {
      let envelope =
        PriorityEnvelope::from_system(SystemMessage::Escalate(info.clone())).map(|sys| (self.map_system)(sys));
      if self.control_ref.sender().try_send(envelope).is_ok() {
        return Ok(());
      }
    }

    Err(info)
  }
}
