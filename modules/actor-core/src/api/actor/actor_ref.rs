use crate::runtime::context::InternalActorRef;
use crate::runtime::message::DynMessage;
use crate::SystemMessage;
use crate::{MailboxFactory, PriorityEnvelope};
use core::marker::PhantomData;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

use crate::api::messaging::MessageEnvelope;

#[derive(Clone)]
pub struct ActorRef<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone, {
  inner: InternalActorRef<DynMessage, R>,
  _marker: PhantomData<U>,
}

impl<U, R> ActorRef<U, R>
where
  U: Element,
  R: MailboxFactory + Clone + 'static,
  R::Queue<PriorityEnvelope<DynMessage>>: Clone,
  R::Signal: Clone,
{
  pub(crate) fn new(inner: InternalActorRef<DynMessage, R>) -> Self {
    Self {
      inner,
      _marker: PhantomData,
    }
  }

  fn wrap_user(message: U) -> DynMessage {
    DynMessage::new(MessageEnvelope::User(message))
  }

  pub fn tell(&self, message: U) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self
      .inner
      .try_send_with_priority(Self::wrap_user(message), DEFAULT_PRIORITY)
  }

  pub fn tell_with_priority(&self, message: U, priority: i8) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    self.inner.try_send_with_priority(Self::wrap_user(message), priority)
  }

  pub fn send_system(&self, message: SystemMessage) -> Result<(), QueueError<PriorityEnvelope<DynMessage>>> {
    let envelope =
      PriorityEnvelope::from_system(message.clone()).map(|sys| DynMessage::new(MessageEnvelope::<U>::System(sys)));
    self.inner.try_send_envelope(envelope)
  }
}
