use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::actor::core::ActorError;
use crate::actor::core::ErrorReason;
use crate::actor::dispatch::MailboxQueueKind;
use crate::actor::message::MessageHandle;

// MessageInvoker trait
#[async_trait]
pub trait MessageInvoker: Debug + Send + Sync {
  async fn invoke_system_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError>;
  async fn invoke_user_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError>;
  async fn escalate_failure(&mut self, reason: ErrorReason, message_handle: MessageHandle);
  async fn record_mailbox_queue_latency(&mut self, _queue: MailboxQueueKind, _latency: Duration) {}
}

#[derive(Debug, Clone)]
pub struct MessageInvokerHandle(Arc<RwLock<dyn MessageInvoker>>);

unsafe impl Send for MessageInvokerHandle {}
unsafe impl Sync for MessageInvokerHandle {}

impl MessageInvokerHandle {
  pub fn new(invoker: Arc<RwLock<dyn MessageInvoker>>) -> Self {
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
    (self.0.as_ref() as *const RwLock<dyn MessageInvoker>).hash(state);
  }
}

#[async_trait]
impl MessageInvoker for MessageInvokerHandle {
  async fn invoke_system_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError> {
    let mut mg = self.0.write().await;
    mg.invoke_system_message(message_handle).await
  }

  async fn invoke_user_message(&mut self, message_handle: MessageHandle) -> Result<(), ActorError> {
    let mut mg = self.0.write().await;
    mg.invoke_user_message(message_handle).await
  }

  async fn escalate_failure(&mut self, reason: ErrorReason, message_handle: MessageHandle) {
    let mut mg = self.0.write().await;
    mg.escalate_failure(reason, message_handle).await;
  }

  async fn record_mailbox_queue_latency(&mut self, queue: MailboxQueueKind, latency: Duration) {
    let mut mg = self.0.write().await;
    mg.record_mailbox_queue_latency(queue, latency).await;
  }
}

static_assertions::assert_impl_all!(MessageInvokerHandle: Send, Sync);
