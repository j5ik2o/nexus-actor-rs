use crate::actor::message::message::Message;
use async_trait::async_trait;

#[async_trait]
pub trait Task: Message {
  async fn run(&self);
}
