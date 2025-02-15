use crate::actor::{Context, Message, MessageHandle, MessageOrEnvelope, Pid};
use async_trait::async_trait;
use std::fmt::Debug;

#[async_trait]
pub trait Actor: Debug + Send + Sync + 'static {
  async fn started(&mut self, ctx: &dyn Context) {}
  async fn stopped(&mut self, ctx: &dyn Context) {}
  async fn receive(&mut self, ctx: &dyn Context, msg: MessageOrEnvelope);
}

#[async_trait]
pub trait ActorRef: Debug + Send + Sync + 'static {
  async fn send(&self, message: MessageHandle);
  async fn stop(&self);
  fn pid(&self) -> &Pid;
}
