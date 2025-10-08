use crate::runtime::context::ActorContext;
use crate::supervisor::Supervisor;
use crate::ActorId;
use crate::ActorPath;
use crate::MailboxRuntime;
use crate::PriorityEnvelope;
use crate::SystemMessage;
use nexus_utils_core_rs::{Element, QueueError, DEFAULT_PRIORITY};

use crate::api::messaging::MessageEnvelope;

/// Typed actor execution context wrapper.
/// 'r: lifetime of the mutable reference to ActorContext
/// 'ctx: lifetime parameter of ActorContext itself
pub struct Context<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone, {
  inner: &'r mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
}

impl<'r, 'ctx, U, R> Context<'r, 'ctx, U, R>
where
  U: Element,
  R: MailboxRuntime + Clone + 'static,
  R::Queue<PriorityEnvelope<MessageEnvelope<U>>>: Clone,
  R::Signal: Clone,
{
  pub(super) fn new(
    inner: &'r mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>>,
  ) -> Self {
    Self { inner }
  }

  pub fn actor_id(&self) -> ActorId {
    self.inner.actor_id()
  }

  pub fn actor_path(&self) -> &ActorPath {
    self.inner.actor_path()
  }

  pub fn watchers(&self) -> &[ActorId] {
    self.inner.watchers()
  }

  pub fn send_to_self(&self, message: U) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    self
      .inner
      .send_to_self_with_priority(MessageEnvelope::User(message), DEFAULT_PRIORITY)
  }

  pub fn send_system_to_self(
    &self,
    message: SystemMessage,
  ) -> Result<(), QueueError<PriorityEnvelope<MessageEnvelope<U>>>> {
    let priority = message.priority();
    self
      .inner
      .send_control_to_self(MessageEnvelope::System(message), priority)
  }

  pub fn inner(&mut self) -> &mut ActorContext<'ctx, MessageEnvelope<U>, R, dyn Supervisor<MessageEnvelope<U>>> {
    self.inner
  }
}
