use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{Message, MessageHandle, MessageOrEnvelope};

#[derive(Debug)]
pub struct TypedContextHandle<M: Message> {
  inner: Arc<RwLock<dyn Debug + Send + Sync>>,
  _phantom: PhantomData<M>,
}

impl<M: Message> TypedContextHandle<M> {
  pub fn new(inner: Arc<RwLock<dyn Debug + Send + Sync>>) -> Self {
    Self {
      inner,
      _phantom: PhantomData,
    }
  }

  pub async fn get_message(&self) -> M {
    // Implementation will be added later
    unimplemented!()
  }

  pub async fn get_message_envelope(&self) -> MessageOrEnvelope {
    // Implementation will be added later
    unimplemented!()
  }
}

impl<M: Message> Clone for TypedContextHandle<M> {
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
      _phantom: PhantomData,
    }
  }
}
