use std::any::Any;

use crate::actor::actor::Stop;
use crate::actor::message::message_handle::Message;
use crate::actor::message::messages::{Restart, Started};

#[derive(Debug, Clone)]
pub enum SystemMessage {
  Restart(Restart),
  Started(Started),
  Stop(Stop),
}

impl Message for SystemMessage {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<SystemMessage>();
    match (self, msg) {
      (SystemMessage::Restart(_), Some(&SystemMessage::Restart(_))) => true,
      (SystemMessage::Started(_), Some(&SystemMessage::Started(_))) => true,
      (SystemMessage::Stop(_), Some(&SystemMessage::Stop(_))) => true,
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
