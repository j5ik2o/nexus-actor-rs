use async_trait::async_trait;

use crate::actor::message::Message;

#[async_trait]
pub trait Task: Message {
  async fn run(&self);
}
