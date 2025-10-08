use crate::context::PriorityActorRef;
use crate::mailbox::SystemMessage;
use crate::{MailboxRuntime, PriorityEnvelope};
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

use super::MessageEnvelope;

#[derive(Clone)]
pub struct TypedActorRef<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone, {
  inner: PriorityActorRef<MessageEnvelope<U>, R>,
}

impl<U, R> TypedActorRef<U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub fn new(inner: PriorityActorRef<MessageEnvelope<U>, R>) -> Self {
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

  pub fn inner(&self) -> &PriorityActorRef<MessageEnvelope<U>, R> {
    &self.inner
  }
}
