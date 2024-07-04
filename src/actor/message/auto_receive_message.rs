use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::PoisonPill;
use crate::actor::message::message_handle::{Message, MessageHandle};
use crate::actor::messages::{Restarting, Stopped, Stopping};
use std::any::Any;

#[derive(Debug, Clone)]
pub enum AutoReceiveMessage {
  Restarting(Restarting),
  Stopping(Stopping),
  Stopped(Stopped),
  PoisonPill(PoisonPill),
}

impl Message for AutoReceiveMessage {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<AutoReceiveMessage>();
    match (self, msg) {
      (AutoReceiveMessage::Restarting(_), Some(&AutoReceiveMessage::Restarting(_))) => true,
      (AutoReceiveMessage::Stopping(_), Some(&AutoReceiveMessage::Stopping(_))) => true,
      (AutoReceiveMessage::Stopped(_), Some(&AutoReceiveMessage::Stopped(_))) => true,
      (AutoReceiveMessage::PoisonPill(_), Some(&AutoReceiveMessage::PoisonPill(_))) => true,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

impl PartialEq for AutoReceiveMessage {
  fn eq(&self, other: &Self) -> bool {
    self.eq_message(other)
  }
}

impl AutoReceiveMessage {
  pub fn auto_receive_message(&self, pid: &ExtendedPid, message: MessageHandle) {}
}
