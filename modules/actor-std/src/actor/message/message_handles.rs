use crate::runtime::TokioMutex;
use nexus_actor_core_rs::runtime::AsyncMutex;
use std::sync::Arc;

use crate::actor::message::message_handle::MessageHandle;

#[derive(Debug, Clone)]
pub struct MessageHandles(Arc<TokioMutex<Vec<MessageHandle>>>);

impl MessageHandles {
  pub fn new(msgs: impl IntoIterator<Item = MessageHandle>) -> Self {
    Self(Arc::new(TokioMutex::new(msgs.into_iter().collect())))
  }

  pub async fn push(&self, msg: MessageHandle) {
    self.0.lock().await.push(msg);
  }

  pub async fn pop(&self) -> Option<MessageHandle> {
    self.0.lock().await.pop()
  }

  pub async fn len(&self) -> usize {
    self.0.lock().await.len()
  }

  pub async fn is_empty(&self) -> bool {
    self.0.lock().await.is_empty()
  }

  pub async fn clear(&self) {
    self.0.lock().await.clear();
  }

  pub async fn to_values(&self) -> Vec<MessageHandle> {
    self.0.lock().await.clone()
  }
}

impl Default for MessageHandles {
  fn default() -> Self {
    Self::new(vec![])
  }
}
