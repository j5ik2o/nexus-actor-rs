use crate::actor::context::context_snapshot::ContextSnapshot;
use crate::actor::message::MessageEnvelope;
use nexus_actor_core_rs::context::CoreActorContextSnapshot;

#[derive(Debug, Clone)]
pub struct ReceiverSnapshot {
  pub(crate) context: ContextSnapshot,
  pub(crate) message: MessageEnvelope,
}

impl ReceiverSnapshot {
  pub fn new(context: ContextSnapshot, message: MessageEnvelope) -> Self {
    Self { context, message }
  }

  pub fn context(&self) -> &ContextSnapshot {
    &self.context
  }

  pub fn message(&self) -> &MessageEnvelope {
    &self.message
  }

  pub fn core_snapshot(&self) -> Option<&CoreActorContextSnapshot> {
    self.context.core_snapshot()
  }

  pub fn map_message<F>(mut self, f: F) -> Self
  where
    F: FnOnce(MessageEnvelope) -> MessageEnvelope, {
    self.message = f(self.message);
    self
  }

  pub fn into_parts(self) -> (ContextSnapshot, MessageEnvelope) {
    (self.context, self.message)
  }
}
