#![cfg(feature = "alloc")]

use alloc::sync::Arc;
use core::any::Any;
use core::time::Duration;

use crate::actor::core_types::message_envelope::CoreMessageEnvelope;
use crate::actor::core_types::message_handle::MessageHandle;
use crate::actor::core_types::message_headers::ReadonlyMessageHeadersHandle;
use crate::actor::core_types::pid::CorePid;
use crate::context::core::{CoreActorContextBuilder, CoreActorContextSnapshot};

#[derive(Debug, Clone, Default)]
pub struct CoreContextSnapshot {
  actor: Option<Arc<dyn Any + Send + Sync>>, // placeholder for future extensions
  parent: Option<CorePid>,
  self_pid: Option<CorePid>,
  sender: Option<CorePid>,
  message_envelope: Option<CoreMessageEnvelope>,
  message_handle: Option<MessageHandle>,
  message_header: Option<ReadonlyMessageHeadersHandle>,
  receive_timeout: Option<Duration>,
}

impl CoreContextSnapshot {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use]
  pub fn with_self_pid(mut self, pid: Option<CorePid>) -> Self {
    self.self_pid = pid;
    self
  }

  #[must_use]
  pub fn with_sender(mut self, sender: Option<CorePid>) -> Self {
    self.sender = sender;
    self
  }

  #[must_use]
  pub fn with_parent(mut self, parent: Option<CorePid>) -> Self {
    self.parent = parent;
    self
  }

  #[must_use]
  pub fn with_message_envelope(mut self, envelope: Option<CoreMessageEnvelope>) -> Self {
    self.message_envelope = envelope;
    self
  }

  #[must_use]
  pub fn with_message_handle(mut self, handle: Option<MessageHandle>) -> Self {
    self.message_handle = handle;
    self
  }

  #[must_use]
  pub fn with_message_header(mut self, header: Option<ReadonlyMessageHeadersHandle>) -> Self {
    self.message_header = header;
    self
  }

  #[must_use]
  pub fn with_receive_timeout(mut self, timeout: Option<Duration>) -> Self {
    self.receive_timeout = timeout;
    self
  }

  #[must_use]
  pub fn with_actor(mut self, actor: Option<Arc<dyn Any + Send + Sync>>) -> Self {
    self.actor = actor;
    self
  }

  #[must_use]
  pub fn self_pid(&self) -> Option<&CorePid> {
    self.self_pid.as_ref()
  }

  #[must_use]
  pub fn sender_pid(&self) -> Option<&CorePid> {
    self.sender.as_ref()
  }

  #[must_use]
  pub fn message_envelope(&self) -> Option<&CoreMessageEnvelope> {
    self.message_envelope.as_ref()
  }

  #[must_use]
  pub fn message_handle(&self) -> Option<&MessageHandle> {
    self.message_handle.as_ref()
  }

  #[must_use]
  pub fn message_header(&self) -> Option<&ReadonlyMessageHeadersHandle> {
    self.message_header.as_ref()
  }

  #[must_use]
  pub fn receive_timeout(&self) -> Option<Duration> {
    self.receive_timeout
  }

  #[must_use]
  pub fn parent_pid(&self) -> Option<&CorePid> {
    self.parent.as_ref()
  }

  #[must_use]
  pub fn actor_arc(&self) -> Option<Arc<dyn Any + Send + Sync>> {
    self.actor.clone()
  }

  #[must_use]
  pub fn actor_as<T>(&self) -> Option<&T>
  where
    T: Any + Send + Sync + 'static, {
    self.actor.as_ref()?.as_ref().downcast_ref::<T>()
  }

  #[must_use]
  pub fn build(self) -> Option<CoreActorContextSnapshot> {
    let self_pid = self.self_pid?;
    Some(
      CoreActorContextBuilder::new(self_pid)
        .with_sender(self.sender)
        .with_message(self.message_handle)
        .with_headers(self.message_header)
        .with_receive_timeout(self.receive_timeout)
        .build(),
    )
  }
}

impl From<CoreActorContextSnapshot> for CoreContextSnapshot {
  fn from(snapshot: CoreActorContextSnapshot) -> Self {
    CoreContextSnapshot::from(&snapshot)
  }
}

impl From<&CoreActorContextSnapshot> for CoreContextSnapshot {
  fn from(snapshot: &CoreActorContextSnapshot) -> Self {
    CoreContextSnapshot::new()
      .with_self_pid(Some(snapshot.self_pid_core()))
      .with_sender(snapshot.sender_pid_core())
      .with_message_handle(snapshot.message_handle())
      .with_message_header(snapshot.headers_handle())
      .with_receive_timeout(snapshot.receive_timeout())
  }
}

#[derive(Debug, Clone)]
pub struct CoreReceiverSnapshot {
  context: CoreContextSnapshot,
  message: CoreMessageEnvelope,
}

impl CoreReceiverSnapshot {
  #[must_use]
  pub fn new(context: CoreContextSnapshot, message: CoreMessageEnvelope) -> Self {
    Self { context, message }
  }

  #[must_use]
  pub fn context(&self) -> &CoreContextSnapshot {
    &self.context
  }

  #[must_use]
  pub fn message(&self) -> &CoreMessageEnvelope {
    &self.message
  }

  #[must_use]
  pub fn into_core_invocation(self) -> Option<crate::context::core::CoreReceiverInvocation> {
    let snapshot = self.context.build()?;
    Some(crate::context::core::CoreReceiverInvocation::new(
      snapshot,
      self.message,
    ))
  }
}
