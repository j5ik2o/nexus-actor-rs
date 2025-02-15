use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;

use crate::actor::{event_stream::EventStream, ActorSystem, Message, MessageHandle, Pid, Process};

#[derive(Debug)]
pub struct SupervisionEvent {
  pub who: Option<Pid>,
  pub message: Option<MessageHandle>,
}

impl Message for SupervisionEvent {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

impl SupervisionEvent {
  pub async fn publish(self, actor_system: &ActorSystem) {
    actor_system.event_stream().await.map(|stream| {
      tokio::spawn(async move {
        stream.send_user_message(None, Box::new(self)).await;
      });
    });
  }
}
