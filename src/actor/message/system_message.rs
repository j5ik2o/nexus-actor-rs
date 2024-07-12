use std::any::Any;

use crate::actor::message::message::Message;
use crate::actor::message::watch::{Unwatch, Watch};

#[derive(Debug, Clone)]
pub enum SystemMessage {
  Restart,
  Start,
  Stop,
  Watch(Watch),
  Unwatch(Unwatch),
}

impl Message for SystemMessage {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<SystemMessage>();
    match (self, msg) {
      (SystemMessage::Restart, Some(&SystemMessage::Restart)) => true,
      (SystemMessage::Start, Some(&SystemMessage::Start)) => true,
      (SystemMessage::Stop, Some(&SystemMessage::Stop)) => true,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

impl SystemMessage {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn system_message(&self) {}
}
