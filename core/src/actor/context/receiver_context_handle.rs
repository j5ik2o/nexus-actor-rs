use std::sync::Arc;

use async_trait::async_trait;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::actor_context::{ActorContext, ContextBorrow};
use crate::actor::context::context_handle::{ContextCellStats, ContextHandle};
use crate::actor::context::context_snapshot::ContextSnapshot;
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

  pub fn with_actor_borrow<R, F>(&self, f: F) -> Option<R>
  where
    F: for<'a> FnOnce(ContextBorrow<'a>) -> R, {
    self.context.with_actor_borrow(f)
  }

  pub fn context_cell_stats(&self) -> ContextCellStats {
    self.context.context_cell_stats()
  }

  pub fn try_message_envelope(&self) -> Option<MessageEnvelope> {
    self.context.try_get_message_envelope_opt()
  }

  pub fn try_message_handle(&self) -> Option<MessageHandle> {
    self.context.try_get_message_handle_opt()
  }

  pub fn try_message_header(&self) -> Option<ReadonlyMessageHeadersHandle> {
    self.context.try_get_message_header_handle()
  }

  pub fn snapshot(&self) -> ContextSnapshot {
    self.context.snapshot()
  }

  pub fn try_sender(&self) -> Option<ExtendedPid> {
    self.context.try_get_sender_opt()
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::context_handle::ContextHandle;
  use crate::actor::core::ActorError;
  use crate::actor::core::Props;
  use crate::actor::message::Message;
  use std::any::Any;

  #[derive(Debug, Clone, PartialEq, Eq)]
  struct TestMessage;

  impl Message for TestMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other.as_any().downcast_ref::<Self>().is_some()
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }

    fn get_type_name(&self) -> String {
      "TestMessage".to_string()
    }
  }

  #[tokio::test]
  async fn try_message_access_uses_snapshot() {
    let actor_system = ActorSystem::new().await.expect("actor system");
    let props = Props::from_async_actor_receiver(|_ctx| async { Ok::<(), ActorError>(()) }).await;
    let actor_context = ActorContext::new(actor_system, props, None).await;
    actor_context
      .inject_message_for_test(MessageHandle::new(TestMessage))
      .await;

    let context_handle = ContextHandle::new(actor_context.clone());
    let receiver_handle = ReceiverContextHandle::new(context_handle);

    assert!(receiver_handle.try_message_handle().is_some());
    assert!(receiver_handle.try_message_envelope().is_none());
    assert!(receiver_handle.try_message_header().is_none());

    let _ = receiver_handle.with_actor_borrow(|borrow| borrow.self_pid().cloned());

    actor_context.clear_message_for_test().await;
    assert!(receiver_handle.try_message_handle().is_none());
  }
}
