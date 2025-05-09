use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart};
use crate::actor::core::ActorError;
use crate::actor::core::ActorHandle;
use crate::actor::core::ExtendedPid;
use crate::actor::message::MessageEnvelope;
use crate::actor::message::MessageHandle;
use crate::actor::message::ReadonlyMessageHeadersHandle;
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};

#[derive(Debug, Clone)]
pub struct ReceiverContextHandle(Arc<RwLock<dyn ReceiverContext>>);

impl ReceiverContextHandle {
  pub fn new_arc(context: Arc<RwLock<dyn ReceiverContext>>) -> Self {
    ReceiverContextHandle(context)
  }

  pub fn new(c: impl ReceiverContext + 'static) -> Self {
    ReceiverContextHandle(Arc::new(RwLock::new(c)))
  }
}

#[async_trait]
impl InfoPart for ReceiverContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    let mg = self.0.read().await;
    mg.get_parent().await
  }

  async fn get_self_opt(&self) -> Option<ExtendedPid> {
    let mg = self.0.read().await;
    mg.get_self_opt().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut mg = self.0.write().await;
    mg.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let mg = self.0.read().await;
    mg.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let mg = self.0.read().await;
    mg.get_actor_system().await
  }
}

#[async_trait]
impl ReceiverPart for ReceiverContextHandle {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError> {
    let mut mg = self.0.write().await;
    mg.receive(envelope).await
  }
}

#[async_trait]
impl MessagePart for ReceiverContextHandle {
  async fn get_message_envelope_opt(&self) -> Option<MessageEnvelope> {
    let mg = self.0.read().await;
    let result = mg.get_message_envelope_opt().await;
    result
  }

  async fn get_message_handle_opt(&self) -> Option<MessageHandle> {
    let mg = self.0.read().await;
    let result = mg.get_message_handle_opt().await;
    result
  }

  async fn get_message_header_handle(&self) -> Option<ReadonlyMessageHeadersHandle> {
    let mg = self.0.read().await;
    mg.get_message_header_handle().await
  }
}

#[async_trait]
impl ExtensionPart for ReceiverContextHandle {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    let mut mg = self.0.write().await;
    mg.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    let mut mg = self.0.write().await;
    mg.set(ext).await
  }
}

impl ReceiverContext for ReceiverContextHandle {}
