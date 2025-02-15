use crate::actor::{Message, MessageHandle};
use async_trait::async_trait;
use std::fmt::Debug;

#[async_trait]
pub trait ActorContext: Debug + Send + Sync + 'static {
  async fn get_message(&self) -> MessageHandle;
  fn as_context(&self) -> Option<Box<dyn ActorContext>>;
}
