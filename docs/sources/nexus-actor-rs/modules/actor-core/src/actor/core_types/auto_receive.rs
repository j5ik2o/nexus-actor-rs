#![cfg(feature = "alloc")]

use alloc::string::String;

use super::message::Message;
use super::message::NotInfluenceReceiveTimeout;
use super::message::TerminateReason;
use super::pid::CorePid;
use alloc::fmt;
use core::any::Any;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminatedMessage {
  pub who: Option<CorePid>,
  pub reason: TerminateReason,
}

impl TerminatedMessage {
  #[must_use]
  pub const fn new(who: Option<CorePid>, reason: TerminateReason) -> Self {
    Self { who, reason }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoReceiveMessage {
  PreStart,
  PostStart,
  PreRestart,
  PostRestart,
  PreStop,
  PostStop,
  Terminated(TerminatedMessage),
}

impl Message for AutoReceiveMessage {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other
      .as_any()
      .downcast_ref::<AutoReceiveMessage>()
      .map_or(false, |value| value == self)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    match self {
      AutoReceiveMessage::PreStart => "AutoReceiveMessage::PreStart".into(),
      AutoReceiveMessage::PostStart => "AutoReceiveMessage::PostStart".into(),
      AutoReceiveMessage::PreRestart => "AutoReceiveMessage::PreRestart".into(),
      AutoReceiveMessage::PostRestart => "AutoReceiveMessage::PostRestart".into(),
      AutoReceiveMessage::PreStop => "AutoReceiveMessage::PreStop".into(),
      AutoReceiveMessage::PostStop => "AutoReceiveMessage::PostStop".into(),
      AutoReceiveMessage::Terminated(_) => "AutoReceiveMessage::Terminated".into(),
    }
  }
}

impl NotInfluenceReceiveTimeout for AutoReceiveMessage {}

impl fmt::Display for AutoReceiveMessage {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
