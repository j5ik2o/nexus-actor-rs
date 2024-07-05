use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::PoisonPill;
use crate::actor::message::message::Message;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::messages::{Restarting, Stopped, Stopping};
use std::any::Any;
use std::fmt::Display;

#[derive(Debug, Clone)]
pub enum AutoReceiveMessage {
  Restarting(Restarting),
  Stopping(Stopping),
  Stopped(Stopped),
  PoisonPill(PoisonPill),
}

impl AutoReceiveMessage {
  pub fn auto_receive_message(&self, _: &ExtendedPid, _: MessageHandle) {}
}

static_assertions::assert_impl_all!(AutoReceiveMessage: Send, Sync);

impl PartialEq for AutoReceiveMessage {
  fn eq(&self, other: &Self) -> bool {
    self.eq_message(other)
  }
}

impl Eq for AutoReceiveMessage {}

impl Display for AutoReceiveMessage {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      AutoReceiveMessage::Restarting(_) => write!(f, "Restarting"),
      AutoReceiveMessage::Stopping(_) => write!(f, "Stopping"),
      AutoReceiveMessage::Stopped(_) => write!(f, "Stopped"),
      AutoReceiveMessage::PoisonPill(_) => write!(f, "PoisonPill"),
    }
  }
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
