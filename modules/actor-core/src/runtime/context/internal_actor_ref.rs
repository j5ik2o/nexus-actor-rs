use crate::SystemMessage;
use crate::{MailboxFactory, PriorityEnvelope, QueueMailboxProducer};
use nexus_utils_core_rs::{Element, QueueError};

/// Actor reference. Wraps QueueMailboxProducer and provides message sending API.
pub(crate) struct InternalActorRef<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone, {
  sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>,
}

unsafe impl<M, R> Send for InternalActorRef<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone + Send + Sync,
  R::Signal: Clone + Send + Sync,
{
}

unsafe impl<M, R> Sync for InternalActorRef<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone + Send + Sync,
  R::Signal: Clone + Send + Sync,
{
}

impl<M, R> Clone for InternalActorRef<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  fn clone(&self) -> Self {
    Self {
      sender: self.sender.clone(),
    }
  }
}

impl<M, R> InternalActorRef<M, R>
where
  M: Element,
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<M>>: Clone,
  R::Signal: Clone,
{
  pub fn new(sender: QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal>) -> Self {
    Self { sender }
  }

  pub fn try_send_with_priority(&self, message: M, priority: i8) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.sender.try_send(PriorityEnvelope::new(message, priority))
  }

  #[allow(dead_code)]
  pub fn try_send_control_with_priority(
    &self,
    message: M,
    priority: i8,
  ) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.sender.try_send(PriorityEnvelope::control(message, priority))
  }

  #[allow(dead_code)]
  pub fn try_send_envelope(&self, envelope: PriorityEnvelope<M>) -> Result<(), QueueError<PriorityEnvelope<M>>> {
    self.sender.try_send(envelope)
  }

  pub fn sender(&self) -> &QueueMailboxProducer<R::Queue<PriorityEnvelope<M>>, R::Signal> {
    &self.sender
  }
}

impl<R> InternalActorRef<SystemMessage, R>
where
  R: MailboxFactory,
  R::Queue<PriorityEnvelope<SystemMessage>>: Clone,
  R::Signal: Clone,
{
  #[allow(dead_code)]
  pub fn try_send_system(&self, message: SystemMessage) -> Result<(), QueueError<PriorityEnvelope<SystemMessage>>> {
    self.sender.try_send(PriorityEnvelope::from_system(message))
  }
}
