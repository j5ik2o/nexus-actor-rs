use crate::runtime::StdAsyncMutex;
use nexus_actor_core_rs::actor::core_types::message_handle::MessageHandle;
use nexus_actor_core_rs::actor::core_types::message_handles::CoreMessageHandles;

#[derive(Debug, Clone)]
pub struct MessageHandles(CoreMessageHandles<StdAsyncMutex<Vec<MessageHandle>>>);

impl MessageHandles {
  pub fn new(msgs: impl IntoIterator<Item = MessageHandle>) -> Self {
    let initial = msgs.into_iter().collect::<Vec<_>>();
    Self(CoreMessageHandles::from_mutex(StdAsyncMutex::new(initial)))
  }

  pub fn from_core(handles: CoreMessageHandles<StdAsyncMutex<Vec<MessageHandle>>>) -> Self {
    Self(handles)
  }

  pub fn inner(&self) -> CoreMessageHandles<StdAsyncMutex<Vec<MessageHandle>>> {
    self.0.clone()
  }

  pub async fn push(&self, msg: MessageHandle) {
    self.0.push(msg).await;
  }

  pub async fn pop(&self) -> Option<MessageHandle> {
    self.0.pop().await
  }

  pub async fn len(&self) -> usize {
    self.0.len().await
  }

  pub async fn is_empty(&self) -> bool {
    self.0.is_empty().await
  }

  pub async fn clear(&self) {
    self.0.clear().await;
  }

  pub async fn to_values(&self) -> Vec<MessageHandle> {
    self.0.to_vec().await
  }
}

impl Default for MessageHandles {
  fn default() -> Self {
    Self::new(Vec::new())
  }
}

impl From<Vec<MessageHandle>> for MessageHandles {
  fn from(value: Vec<MessageHandle>) -> Self {
    Self::new(value)
  }
}
