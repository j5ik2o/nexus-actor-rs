use crate::actor::context::context_snapshot::ContextSnapshot;
use crate::actor::message::MessageEnvelope;
use nexus_actor_core_rs::context::{
  CoreActorContextSnapshot, CoreContextSnapshot, CoreReceiverInvocation, CoreReceiverSnapshot,
};

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

  pub fn core_snapshot(&self) -> Option<CoreActorContextSnapshot> {
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

  pub fn to_core_snapshot(&self) -> Option<CoreReceiverSnapshot> {
    let context = self.context.core_context_snapshot()?;
    Some(CoreReceiverSnapshot::new(context, self.message.clone().into_core()))
  }

  pub fn into_core_snapshot(self) -> Option<CoreReceiverSnapshot> {
    let (context, message) = self.into_parts();
    let core_context = context.into_core_context_snapshot()?;
    Some(CoreReceiverSnapshot::new(core_context, message.into_core()))
  }

  pub fn to_core_invocation(&self) -> Option<CoreReceiverInvocation> {
    self.to_core_snapshot()?.into_core_invocation()
  }

  pub fn into_core_invocation(self) -> Option<CoreReceiverInvocation> {
    self.into_core_snapshot()?.into_core_invocation()
  }

  pub fn from_core_invocation(invocation: CoreReceiverInvocation) -> Self {
    let (context_snapshot, envelope) = invocation.into_parts();
    let core_context = CoreContextSnapshot::from(context_snapshot);
    let context = ContextSnapshot::default().with_core_snapshot(core_context);
    let message = MessageEnvelope::from_core(envelope);
    Self::new(context, message)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::message::Message;
  use nexus_actor_core_rs::context::CoreContextSnapshot;
  use nexus_actor_core_rs::{CorePid, MessageHandle};
  use std::any::Any;

  #[derive(Debug, Clone, PartialEq)]
  struct TestMessage(&'static str);

  impl Message for TestMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other
        .as_any()
        .downcast_ref::<TestMessage>()
        .map_or(false, |value| value == self)
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }

    fn get_type_name(&self) -> String {
      "TestMessage".to_string()
    }
  }

  #[test]
  fn convert_to_core_invocation() {
    let core_snapshot = CoreActorContextSnapshot::new(
      CorePid::new("node", "actor"),
      None,
      Some(MessageHandle::new(TestMessage("msg"))),
      None,
    );

    let context_snapshot = ContextSnapshot::default().with_core_snapshot(CoreContextSnapshot::from(&core_snapshot));
    let envelope = MessageEnvelope::new(MessageHandle::new(TestMessage("msg")));
    let snapshot = ReceiverSnapshot::new(context_snapshot, envelope.clone());

    let invocation = snapshot.to_core_invocation().expect("core invocation");
    assert_eq!(invocation.context().self_pid_core(), core_snapshot.self_pid_core());
    assert_eq!(
      invocation.envelope().message_handle().type_id(),
      envelope.get_message_handle().type_id()
    );
  }
}
