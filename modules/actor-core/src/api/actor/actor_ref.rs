use crate::runtime::context::InternalActorRef;
use crate::SystemMessage;
use crate::{MailboxFactory, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

use crate::api::messaging::MessageEnvelope;

#[derive(Clone)]
pub struct ActorRef<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone, {
  inner: InternalActorRef<MessageEnvelope<U>, R>,
}

impl<U, R> ActorRef<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub(crate) fn new(inner: InternalActorRef<MessageEnvelope<U>, R>) -> Self {
    Self { inner }
  }

  pub fn tell(&self, message: U) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self
      .inner
      .try_send_with_priority(MessageEnvelope::User(message), DEFAULT_PRIORITY)
  }

  pub fn tell_with_priority(
    &self,
    message: U,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self
      .inner
      .try_send_with_priority(MessageEnvelope::User(message), priority)
  }

  pub fn send_system(&self, message: SystemMessage) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    let priority = message.priority();
    self
      .inner
      .try_send_control_with_priority(MessageEnvelope::System(message), priority)
  }
}
