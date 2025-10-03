use core::mem;
use std::any::Any;
use std::sync::Arc;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::ActorContext;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::ContextBorrow;
use crate::actor::core::{ActorHandle, ExtendedPid};
use crate::actor::message::{MessageEnvelope, MessageHandle, ReadonlyMessageHeadersHandle};
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::context::{CoreActorContextSnapshot, CoreContextSnapshot};

#[derive(Debug, Clone)]
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
  core_snapshot: CoreContextSnapshot,
}

impl Default for ContextSnapshot {
  fn default() -> Self {
    Self {
      actor_system: None,
      actor: None,
      parent: None,
      self_pid: None,
      sender: None,
      message_envelope: None,
      message_handle: None,
      message_header: None,
      context_handle: None,
      core_snapshot: CoreContextSnapshot::new(),
    }
  }
}

impl ContextSnapshot {
  pub fn from_borrow(borrow: &ContextBorrow<'_>) -> Self {
    Self::default()
      .with_actor_system_opt(Some(borrow.actor_system().clone()))
      .with_actor_opt(borrow.actor().cloned())
      .with_parent_opt(borrow.parent().cloned())
      .with_self_pid_opt(borrow.self_pid().cloned())
  }

  pub fn from_actor_context(actor_context: &ActorContext) -> Self {
    Self::from_borrow(&actor_context.borrow())
      .with_sender_opt(actor_context.try_sender())
      .with_message_envelope_opt(actor_context.try_message_envelope())
      .with_message_handle_opt(actor_context.try_message_handle())
      .with_message_header_opt(actor_context.try_message_header())
  }

  pub fn from_context_handle(context_handle: &ContextHandle) -> Self {
    if let Some(actor_context) = context_handle.actor_context_arc() {
      Self::from_actor_context(actor_context.as_ref()).with_context_handle(context_handle.clone())
    } else {
      Self::default().with_context_handle(context_handle.clone())
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

  pub fn core_snapshot(&self) -> Option<CoreActorContextSnapshot> {
    self.core_context_snapshot()?.build()
  }

  pub fn with_actor_system_opt(mut self, actor_system: Option<ActorSystem>) -> Self {
    self.actor_system = actor_system;
    self
  }

  pub fn with_actor_opt(mut self, actor: Option<ActorHandle>) -> Self {
    let core_actor: Option<Arc<dyn Any + Send + Sync>> = actor.as_ref().map(|handle| {
      let typed: Arc<ActorHandle> = Arc::new(handle.clone());
      typed as Arc<dyn Any + Send + Sync>
    });
    self.core_snapshot = mem::take(&mut self.core_snapshot).with_actor(core_actor);
    self.actor = actor;
    self
  }

  pub fn with_parent_opt(mut self, parent: Option<ExtendedPid>) -> Self {
    let core = mem::take(&mut self.core_snapshot);
    self.core_snapshot = core.with_parent(parent.as_ref().map(CorePid::from));
    self.parent = parent;
    self
  }

  pub fn with_self_pid_opt(mut self, self_pid: Option<ExtendedPid>) -> Self {
    let core = mem::take(&mut self.core_snapshot);
    self.core_snapshot = core.with_self_pid(self_pid.as_ref().map(CorePid::from));
    self.self_pid = self_pid;
    self
  }

  pub fn with_sender_opt(mut self, sender: Option<ExtendedPid>) -> Self {
    let core = mem::take(&mut self.core_snapshot);
    self.core_snapshot = core.with_sender(sender.as_ref().map(CorePid::from));
    self.sender = sender;
    self
  }

  pub fn with_message_envelope_opt(mut self, envelope: Option<MessageEnvelope>) -> Self {
    let core_envelope = envelope.as_ref().map(|env| env.clone().into_core());
    let core = mem::take(&mut self.core_snapshot);
    self.core_snapshot = core.with_message_envelope(core_envelope);
    self.message_envelope = envelope;
    self
  }

  pub fn with_message_handle_opt(mut self, handle: Option<MessageHandle>) -> Self {
    let core = mem::take(&mut self.core_snapshot);
    self.core_snapshot = core.with_message_handle(handle.clone());
    self.message_handle = handle;
    self
  }

  pub fn with_message_header_opt(mut self, header: Option<ReadonlyMessageHeadersHandle>) -> Self {
    let core = mem::take(&mut self.core_snapshot);
    self.core_snapshot = core.with_message_header(header.clone());
    self.message_header = header;
    self
  }

  pub fn with_context_handle_opt(mut self, handle: Option<ContextHandle>) -> Self {
    self.context_handle = handle;
    self
  }

  pub fn with_core_snapshot(mut self, snapshot: CoreContextSnapshot) -> Self {
    self.self_pid = snapshot.self_pid().map(ExtendedPid::from);
    self.parent = snapshot.parent_pid().cloned().map(ExtendedPid::from);
    self.sender = snapshot.sender_pid().map(ExtendedPid::from);
    self.message_handle = snapshot.message_handle().cloned();
    self.message_header = snapshot.message_header().cloned();
    self.message_envelope = snapshot.message_envelope().cloned().map(MessageEnvelope::from_core);
    if let Some(actor_arc) = snapshot.actor_arc() {
      if let Ok(actor_handle_arc) = Arc::downcast::<ActorHandle>(actor_arc) {
        self.actor = Some((*actor_handle_arc).clone());
      }
    }
    self.core_snapshot = snapshot;
    self
  }

  pub fn with_core_snapshot_opt(mut self, snapshot: Option<CoreContextSnapshot>) -> Self {
    if let Some(snapshot) = snapshot {
      self = self.with_core_snapshot(snapshot);
    } else {
      self.core_snapshot = CoreContextSnapshot::new();
    }
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

  pub fn core_context_snapshot(&self) -> Option<CoreContextSnapshot> {
    let builder = apply_self_pid(self.core_snapshot.clone(), self.self_pid.as_ref());
    if builder.self_pid().is_some() {
      Some(builder)
    } else {
      None
    }
  }

  pub fn into_core_context_snapshot(self) -> Option<CoreContextSnapshot> {
    let ContextSnapshot {
      self_pid,
      core_snapshot,
      ..
    } = self;
    let builder = apply_self_pid(core_snapshot, self_pid.as_ref());
    if builder.self_pid().is_some() {
      Some(builder)
    } else {
      None
    }
  }

  pub fn into_core_snapshot(self) -> Option<CoreActorContextSnapshot> {
    self.into_core_context_snapshot()?.build()
  }
}

fn apply_self_pid(snapshot: CoreContextSnapshot, self_pid: Option<&ExtendedPid>) -> CoreContextSnapshot {
  if snapshot.self_pid().is_some() {
    snapshot
  } else if let Some(pid) = self_pid {
    snapshot.with_self_pid(Some(CorePid::from(pid)))
  } else {
    snapshot
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::context::ContextHandle;
  use crate::actor::core::{Actor, ActorError, ActorHandle, ExtendedPid};
  use async_trait::async_trait;
  use nexus_actor_core_rs::actor::core_types::pid::CorePid;
  use nexus_actor_core_rs::context::CoreContextSnapshot;

  #[derive(Debug)]
  struct DummyActor;

  #[async_trait]
  impl Actor for DummyActor {
    async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
      Ok(())
    }
  }

  #[test]
  fn core_snapshot_uses_fallback_self_pid() {
    let core_pid = CorePid::new("addr", "id");
    let snapshot = ContextSnapshot::default().with_self_pid_opt(Some(ExtendedPid::from_core(core_pid.clone())));
    let core_snapshot = snapshot.core_snapshot().expect("core snapshot");
    assert_eq!(core_snapshot.self_pid_core(), core_pid);
  }

  #[test]
  fn core_snapshot_preserves_actor_handle() {
    let actor_handle = ActorHandle::new(DummyActor);
    let snapshot = ContextSnapshot::default()
      .with_self_pid_opt(Some(ExtendedPid::from_core(CorePid::new("addr", "id"))))
      .with_actor_opt(Some(actor_handle.clone()));
    let core_snapshot = snapshot.core_context_snapshot().expect("core snapshot");
    let recovered = core_snapshot
      .actor_as::<ActorHandle>()
      .expect("actor handle in core snapshot");
    assert_eq!(recovered.type_name(), actor_handle.type_name());
  }

  #[test]
  fn applying_core_snapshot_restores_actor_and_parent() {
    let core_pid = CorePid::new("parent", "child");
    let actor_handle = ActorHandle::new(DummyActor);
    let core_actor: Arc<dyn Any + Send + Sync> = Arc::new(actor_handle.clone());
    let core_snapshot = CoreContextSnapshot::new()
      .with_self_pid(Some(CorePid::new("addr", "id")))
      .with_parent(Some(core_pid.clone()))
      .with_actor(Some(core_actor));
    let snapshot = ContextSnapshot::default().with_core_snapshot(core_snapshot);
    let restored_parent = snapshot.parent().expect("parent restored");
    assert_eq!(restored_parent.to_core(), core_pid);
    let restored_actor = snapshot.actor().expect("actor restored");
    assert_eq!(restored_actor.type_name(), actor_handle.type_name());
  }
}
