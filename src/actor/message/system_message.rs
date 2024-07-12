use std::any::Any;

use crate::actor::message::message::Message;

#[derive(Debug, Clone)]
pub enum SystemMessage {
  Restart,
  Started,
  Stop,
}

impl Message for SystemMessage {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<SystemMessage>();
    match (self, msg) {
      (SystemMessage::Restart, Some(&SystemMessage::Restart)) => true,
      (SystemMessage::Started, Some(&SystemMessage::Started)) => true,
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
