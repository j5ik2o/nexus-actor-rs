use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::ActorContext;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::ContextBorrow;
use crate::actor::core::{ActorHandle, ExtendedPid};
use crate::actor::message::{MessageEnvelope, MessageHandle, ReadonlyMessageHeadersHandle};

#[derive(Debug, Clone, Default)]
pub struct ContextSnapshot {
  actor_system: Option<ActorSystem>,
  actor: Option<ActorHandle>,
  parent: Option<ExtendedPid>,
  self_pid: Option<ExtendedPid>,
  sender: Option<ExtendedPid>,
  message_envelope: Option<MessageEnvelope>,
  message_handle: Option<MessageHandle>,
  message_header: Option<ReadonlyMessageHeadersHandle>,
  context_handle: Option<ContextHandle>,
}

impl ContextSnapshot {
  pub fn from_borrow(borrow: &ContextBorrow<'_>) -> Self {
    Self {
      actor_system: Some(borrow.actor_system().clone()),
      actor: borrow.actor().cloned(),
      parent: borrow.parent().cloned(),
      self_pid: borrow.self_pid().cloned(),
      ..Self::default()
    }
  }

  pub fn from_actor_context(actor_context: &ActorContext) -> Self {
    let mut snapshot = Self::from_borrow(&actor_context.borrow());
    snapshot.sender = actor_context.try_sender();
    snapshot.message_envelope = actor_context.try_message_envelope();
    snapshot.message_handle = actor_context.try_message_handle();
    snapshot.message_header = actor_context.try_message_header();
    snapshot
  }

  pub fn from_context_handle(context_handle: &ContextHandle) -> Self {
    if let Some(actor_context) = context_handle.actor_context_arc() {
      let mut snapshot = Self::from_actor_context(actor_context.as_ref());
      snapshot.context_handle = Some(context_handle.clone());
      snapshot
    } else {
      let mut snapshot = Self::default();
      snapshot.context_handle = Some(context_handle.clone());
      snapshot
    }
  }

  pub fn with_context_handle(mut self, handle: ContextHandle) -> Self {
    self.context_handle = Some(handle);
    self
  }

  pub fn actor_system(&self) -> Option<&ActorSystem> {
    self.actor_system.as_ref()
  }

  pub fn actor(&self) -> Option<&ActorHandle> {
    self.actor.as_ref()
  }

  pub fn parent(&self) -> Option<&ExtendedPid> {
    self.parent.as_ref()
  }

  pub fn self_pid(&self) -> Option<&ExtendedPid> {
    self.self_pid.as_ref()
  }

  pub fn sender(&self) -> Option<&ExtendedPid> {
    self.sender.as_ref()
  }

  pub fn message_envelope(&self) -> Option<&MessageEnvelope> {
    self.message_envelope.as_ref()
  }

  pub fn message_handle(&self) -> Option<&MessageHandle> {
    self.message_handle.as_ref()
  }

  pub fn message_header(&self) -> Option<&ReadonlyMessageHeadersHandle> {
    self.message_header.as_ref()
  }

  pub fn with_actor_system_opt(mut self, actor_system: Option<ActorSystem>) -> Self {
    self.actor_system = actor_system;
    self
  }

  pub fn with_actor_opt(mut self, actor: Option<ActorHandle>) -> Self {
    self.actor = actor;
    self
  }

  pub fn with_parent_opt(mut self, parent: Option<ExtendedPid>) -> Self {
    self.parent = parent;
    self
  }

  pub fn with_self_pid_opt(mut self, self_pid: Option<ExtendedPid>) -> Self {
    self.self_pid = self_pid;
    self
  }

  pub fn with_sender_opt(mut self, sender: Option<ExtendedPid>) -> Self {
    self.sender = sender;
    self
  }

  pub fn with_message_envelope_opt(mut self, envelope: Option<MessageEnvelope>) -> Self {
    self.message_envelope = envelope;
    self
  }

  pub fn with_message_handle_opt(mut self, handle: Option<MessageHandle>) -> Self {
    self.message_handle = handle;
    self
  }

  pub fn with_message_header_opt(mut self, header: Option<ReadonlyMessageHeadersHandle>) -> Self {
    self.message_header = header;
    self
  }

  pub fn with_context_handle_opt(mut self, handle: Option<ContextHandle>) -> Self {
    self.context_handle = handle;
    self
  }

  pub fn context_handle(&self) -> Option<&ContextHandle> {
    self.context_handle.as_ref()
  }

  pub fn into_actor_system(self) -> Option<ActorSystem> {
    self.actor_system
  }

  pub fn into_context_handle(self) -> Option<ContextHandle> {
    self.context_handle
  }

  pub fn into_message_handle(self) -> Option<MessageHandle> {
    self.message_handle
  }
}
