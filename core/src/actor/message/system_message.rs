use crate::actor::message::message::Message;
use crate::actor::message::terminate_info::TerminateInfo;
use crate::generated::actor::{Unwatch, Watch};
use std::any::Any;

#[derive(Debug, Clone)]
pub enum SystemMessage {
  Restart,
  Start,
  Stop,
  Watch(Watch),
  Unwatch(Unwatch),
  Terminate(TerminateInfo),
}

impl Message for SystemMessage {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<SystemMessage>();
    match (self, msg) {
      (SystemMessage::Restart, Some(&SystemMessage::Restart)) => true,
      (SystemMessage::Start, Some(&SystemMessage::Start)) => true,
      (SystemMessage::Stop, Some(&SystemMessage::Stop)) => true,
      (SystemMessage::Watch(_), Some(&SystemMessage::Watch(_))) => true,
      (SystemMessage::Unwatch(_), Some(&SystemMessage::Unwatch(_))) => true,
      (SystemMessage::Terminate(me), Some(&SystemMessage::Terminate(ref you))) => *me == *you,
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
