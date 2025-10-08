use alloc::sync::Arc;

use crate::failure::FailureInfo;
use crate::mailbox::{PriorityEnvelope, SystemMessage};
use crate::{MailboxRuntime, PriorityActorRef};
use nexus_utils_core_rs::Element;

use super::EscalationSink;

/// 親 Guardian へ `SystemMessage::Escalate` を転送するシンク。
pub struct ParentGuardianSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  control_ref: PriorityActorRef<M, R>,
  map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>,
}

impl<M, R> ParentGuardianSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(control_ref: PriorityActorRef<M, R>, map_system: Arc<dyn Fn(SystemMessage) -> M + Send + Sync>) -> Self {
    Self {
      control_ref,
      map_system,
    }
  }
}

impl<M, R> EscalationSink<M, R> for ParentGuardianSink<M, R>
where
  M: Element,
  R: MailboxRuntime,
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
