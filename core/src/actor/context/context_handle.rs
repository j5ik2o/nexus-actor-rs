use std::fmt::Debug;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::{ActorContext, Message, MessageHandle};

#[derive(Debug)]
pub struct ContextHandle {
  inner: Arc<RwLock<dyn ActorContext>>,
}

impl ContextHandle {
  pub fn new(context: Arc<RwLock<dyn ActorContext>>) -> Self {
    Self { inner: context }
  }

  pub async fn get_message_handle(&self) -> MessageHandle {
    let context = self.inner.read().await;
    context.get_message().await
  }

  pub(crate) async fn to_actor_context(&self) -> Option<Box<dyn ActorContext>> {
    let context = self.inner.read().await;
    context.as_context()
  }
}
