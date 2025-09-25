use std::sync::Arc;

use async_trait::async_trait;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::ActorContext;
use crate::actor::context::context_handle::{ContextCellStats, ContextHandle};
use crate::actor::context::{ExtensionContext, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart};
use crate::actor::core::ActorError;
use crate::actor::core::ActorHandle;
use crate::actor::core::ExtendedPid;
use crate::actor::message::MessageEnvelope;
use crate::actor::message::MessageHandle;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};

#[derive(Debug, Clone)]
pub struct ReceiverContextHandle {
  context: ContextHandle,
}

impl ReceiverContextHandle {
  pub fn new(context: ContextHandle) -> Self {
    ReceiverContextHandle { context }
  }

  pub fn context_handle(&self) -> &ContextHandle {
    &self.context
  }

  pub fn actor_context_arc(&self) -> Option<Arc<ActorContext>> {
    self.context.actor_context_arc()
  }

  pub fn context_cell_stats(&self) -> ContextCellStats {
    self.context.context_cell_stats()
  }
}

impl ExtensionContext for ReceiverContextHandle {}

#[async_trait]
impl InfoPart for ReceiverContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    self.context.get_parent().await
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
    self.context.get_self_opt().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    self.context.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    self.context.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    self.context.get_actor_system().await
  }
}

#[async_trait]
impl ReceiverPart for ReceiverContextHandle {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError> {
    self.context.receive(envelope).await
  }
}

#[async_trait]
impl MessagePart for ReceiverContextHandle {
  async fn get_message_envelope_opt(&self) -> Option<MessageEnvelope> {
    self.context.get_message_envelope_opt().await
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    self.context.get_message_handle_opt().await
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.context.get_message_header_handle().await
  }
}

#[async_trait]
impl ExtensionPart for ReceiverContextHandle {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    self.context.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    self.context.set(ext).await
  }
}

impl ReceiverContext for ReceiverContextHandle {}
