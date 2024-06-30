use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use crate::actor::actor::ActorError;
use crate::actor::message::MessageHandle;
use crate::actor::ReasonHandle;

// MessageInvoker trait
#[async_trait]
pub trait MessageInvoker: Debug + Send + Sync {
  async fn invoke_system_message(&mut self, message: MessageHandle) -> Result<(), ActorError>;
  async fn invoke_user_message(&mut self, message: MessageHandle) -> Result<(), ActorError>;
  async fn escalate_failure(&self, reason: ReasonHandle, message: MessageHandle);
}

#[derive(Debug, Clone)]
pub struct MessageInvokerHandle(Arc<Mutex<dyn MessageInvoker>>);

impl MessageInvokerHandle {
  pub fn new(invoker: Arc<Mutex<dyn MessageInvoker>>) -> Self {
    MessageInvokerHandle(invoker)
  }
}

impl PartialEq for MessageInvokerHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for MessageInvokerHandle {}

impl std::hash::Hash for MessageInvokerHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const Mutex<dyn MessageInvoker>).hash(state);
  }
}

#[async_trait]
impl MessageInvoker for MessageInvokerHandle {
  async fn invoke_system_message(&mut self, message: MessageHandle) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.invoke_system_message(message).await
  }

  async fn invoke_user_message(&mut self, message: MessageHandle) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.invoke_user_message(message).await
  }

  async fn escalate_failure(&self, reason: ReasonHandle, message: MessageHandle) {
    let mg = self.0.lock().await;
    mg.escalate_failure(reason, message).await;
  }
}
