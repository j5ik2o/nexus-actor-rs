use crate::actor::{Context, MessageHandle};
use async_trait::async_trait;
use std::fmt::Debug;

#[async_trait]
pub trait Lifecycle: Debug + Send + Sync + 'static {
  async fn started(&mut self, ctx: &dyn Context);
  async fn stopped(&mut self, ctx: &dyn Context);
  async fn receive(&mut self, ctx: &dyn Context, msg: MessageHandle);
}
