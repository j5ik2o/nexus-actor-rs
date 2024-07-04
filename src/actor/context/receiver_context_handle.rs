use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::{ActorError, ActorHandle};
use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart};
use crate::actor::message::message_envelope::{MessageEnvelope, ReadonlyMessageHeadersHandle};
use crate::actor::message::message_handle::MessageHandle;
use crate::ctxext::extensions::{ContextExtensionHandle, ContextExtensionId};

#[derive(Debug, Clone)]
pub struct ReceiverContextHandle(Arc<Mutex<dyn ReceiverContext>>);

impl ReceiverContextHandle {
  pub fn new_arc(context: Arc<Mutex<dyn ReceiverContext>>) -> Self {
    ReceiverContextHandle(context)
  }

  pub fn new(c: impl ReceiverContext + 'static) -> Self {
    ReceiverContextHandle(Arc::new(Mutex::new(c)))
  }
}

#[async_trait]
impl InfoPart for ReceiverContextHandle {
  async fn get_parent(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_parent().await
  }

  async fn get_self(&self) -> Option<ExtendedPid> {
    let mg = self.0.lock().await;
    mg.get_self().await
  }

  async fn set_self(&mut self, pid: ExtendedPid) {
    let mut mg = self.0.lock().await;
    mg.set_self(pid).await
  }

  async fn get_actor(&self) -> Option<ActorHandle> {
    let mg = self.0.lock().await;
    mg.get_actor().await
  }

  async fn get_actor_system(&self) -> ActorSystem {
    let mg = self.0.lock().await;
    mg.get_actor_system().await
  }
}

#[async_trait]
impl ReceiverPart for ReceiverContextHandle {
  async fn receive(&mut self, envelope: MessageEnvelope) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.receive(envelope).await
  }
}

#[async_trait]
impl MessagePart for ReceiverContextHandle {
  async fn get_message(&self) -> Option<MessageHandle> {
    let mg = self.0.lock().await;
    let result = mg.get_message().await;
    result
  }

  async fn get_message_header(&self) -> ReadonlyMessageHeadersHandle {
    let mg = self.0.lock().await;
    mg.get_message_header().await
  }
}

#[async_trait]
impl ExtensionPart for ReceiverContextHandle {
  async fn get(&mut self, id: ContextExtensionId) -> Option<ContextExtensionHandle> {
    let mut mg = self.0.lock().await;
    mg.get(id).await
  }

  async fn set(&mut self, ext: ContextExtensionHandle) {
    let mut mg = self.0.lock().await;
    mg.set(ext).await
  }
}

impl ReceiverContext for ReceiverContextHandle {}
