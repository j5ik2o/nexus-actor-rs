use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{Message, MessageHandle, MessageOrEnvelope};

#[derive(Debug, Clone)]
pub struct ContextHandle {
  inner: Arc<RwLock<dyn Debug + Send + Sync>>,
}

impl ContextHandle {
  pub fn new(inner: Arc<RwLock<dyn Debug + Send + Sync>>) -> Self {
    Self { inner }
  }

  pub async fn get_message(&self) -> MessageHandle {
    // Implementation will be added later
    unimplemented!()
  }

  pub async fn get_message_envelope(&self) -> MessageOrEnvelope {
    // Implementation will be added later
    unimplemented!()
  }
}
