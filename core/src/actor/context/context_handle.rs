use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{ActorContext, MessageHandle};

#[derive(Debug)]
pub struct ContextHandle {
  inner: Arc<RwLock<dyn ActorContext>>,
}

impl ContextHandle {
  pub fn new(context: Arc<RwLock<dyn ActorContext>>) -> Self {
    Self { inner: context }
  }

  pub async fn get_message_handle(&self) -> MessageHandle {
    self.inner.read().await.get_message_handle().await
  }

  pub(crate) async fn to_actor_context(&self) -> Option<Box<dyn ActorContext>> {
    let mg = self.inner.read().await;
    mg.as_any().downcast_ref::<dyn ActorContext>().map(Box::new)
  }
}
