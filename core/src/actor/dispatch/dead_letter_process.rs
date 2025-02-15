use async_trait::async_trait;
use std::fmt::Debug;

use crate::actor::{event_stream::EventStream, ActorSystem, Message, Pid, Process};

#[derive(Debug)]
pub struct DeadLetterProcess {
  system: ActorSystem,
}

#[async_trait]
impl Process for DeadLetterProcess {
  async fn send_user_message(&self, sender: Option<&Pid>, message: Box<dyn Message>) {
    let event = DeadLetterEvent {
      pid: None,
      message,
      sender: sender.cloned(),
    };
    self.system.event_stream().publish(event).await;
  }

  async fn send_system_message(&self, message: Box<dyn Message>) {
    let event = DeadLetterEvent {
      pid: None,
      message,
      sender: None,
    };
    self.system.event_stream().publish(event).await;
  }

  async fn stop(&self) {}

  async fn set_dead(&self) {}
}

impl DeadLetterProcess {
  pub fn new(system: ActorSystem) -> Self {
    Self { system }
  }
}

#[derive(Debug)]
pub struct DeadLetterEvent {
  pub pid: Option<Pid>,
  pub message: Box<dyn Message>,
  pub sender: Option<Pid>,
}
