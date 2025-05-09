use crate::actor::core::ExtendedPid;
use crate::actor::message::message::Message;
use crate::actor::message::message_handle::MessageHandle;
use crate::generated::actor::Terminated;
use nexus_actor_message_derive_rs::Message;
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Message)]
pub(crate) enum AutoReceiveMessage {
  PreStart,
  PostStart,
  PreRestart,
  PostRestart,
  PreStop,
  PostStop,
  Terminated(Terminated),
}

impl AutoReceiveMessage {
  pub fn auto_receive_message(&self, _: &ExtendedPid, _: MessageHandle) {}
}

static_assertions::assert_impl_all!(AutoReceiveMessage: Send, Sync);

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
