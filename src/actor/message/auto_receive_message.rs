use std::any::Any;
use std::fmt::Display;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::message::message::Message;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::terminate_info::TerminateInfo;

#[derive(Debug, Clone)]
pub enum AutoReceiveMessage {
  PreStart,
  PostStart,
  PreRestart,
  PostRestart,
  PreStop,
  PostStop,
  Terminated(TerminateInfo),
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
      AutoReceiveMessage::PreStart => write!(f, "PreStart"),
      AutoReceiveMessage::PostStart => write!(f, "PostStart"),
      AutoReceiveMessage::PreRestart => write!(f, "PreRestart"),
      AutoReceiveMessage::PostRestart => write!(f, "PostRestart"),
      AutoReceiveMessage::PreStop => write!(f, "PreStop"),
      AutoReceiveMessage::PostStop => write!(f, "PostStop"),
      AutoReceiveMessage::Terminated(_) => write!(f, "Terminated"),
    }
  }
}

impl Message for AutoReceiveMessage {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<AutoReceiveMessage>();
    match (self, msg) {
      (AutoReceiveMessage::PreStart, Some(&AutoReceiveMessage::PreStart)) => true,
      (AutoReceiveMessage::PostStart, Some(&AutoReceiveMessage::PostStart)) => true,
      (AutoReceiveMessage::PreRestart, Some(&AutoReceiveMessage::PreRestart)) => true,
      (AutoReceiveMessage::PostRestart, Some(&AutoReceiveMessage::PostRestart)) => true,
      (AutoReceiveMessage::PreStop, Some(&AutoReceiveMessage::PreStop)) => true,
      (AutoReceiveMessage::PostStop, Some(&AutoReceiveMessage::PostStop)) => true,
      (AutoReceiveMessage::Terminated(me), Some(&AutoReceiveMessage::Terminated(ref you))) => *me == *you,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
