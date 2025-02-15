//! Message handle implementation.

use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::message::Message;

#[derive(Debug, Clone)]
pub struct MessageHandle {
  inner: Arc<RwLock<Box<dyn Message>>>,
}

impl MessageHandle {
  pub fn new(message: Box<dyn Message>) -> Self {
    Self {
      inner: Arc::new(RwLock::new(message)),
    }
  }

  pub async fn to_typed<T: Message>(&self) -> Option<T> {
    let message = self.inner.read().await;
    message.as_any().downcast_ref::<T>().cloned()
  }
}
