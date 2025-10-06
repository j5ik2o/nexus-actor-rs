#![cfg(feature = "alloc")]

use alloc::string::String;

use super::auto_receive::TerminatedMessage;
use super::message::{Message, NotInfluenceReceiveTimeout, TerminateReason};
use super::pid::CorePid;
use core::any::Any;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchMessage {
  watcher: CorePid,
}

impl WatchMessage {
  #[must_use]
  pub fn new(watcher: CorePid) -> Self {
    Self { watcher }
  }

  #[must_use]
  pub fn watcher(&self) -> &CorePid {
    &self.watcher
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnwatchMessage {
  watcher: CorePid,
}

impl UnwatchMessage {
  #[must_use]
  pub fn new(watcher: CorePid) -> Self {
    Self { watcher }
  }

  #[must_use]
  pub fn watcher(&self) -> &CorePid {
    &self.watcher
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemMessage {
  Restart,
  Start,
  Stop,
  Watch(WatchMessage),
  Unwatch(UnwatchMessage),
  Terminate(TerminatedMessage),
}

impl SystemMessage {
  #[must_use]
  pub const fn restart() -> Self {
    Self::Restart
  }

  #[must_use]
  pub const fn start() -> Self {
    Self::Start
  }

  #[must_use]
  pub const fn stop() -> Self {
    Self::Stop
  }

  #[must_use]
  pub fn watch(watcher: CorePid) -> Self {
    Self::Watch(WatchMessage::new(watcher))
  }

  #[must_use]
  pub fn unwatch(watcher: CorePid) -> Self {
    Self::Unwatch(UnwatchMessage::new(watcher))
  }

  #[must_use]
  pub fn terminate(who: Option<CorePid>, reason: TerminateReason) -> Self {
    Self::Terminate(TerminatedMessage::new(who, reason))
  }

  #[must_use]
  pub fn as_str(&self) -> &'static str {
    match self {
      Self::Restart => "Restart",
      Self::Start => "Start",
      Self::Stop => "Stop",
      Self::Watch(_) => "Watch",
      Self::Unwatch(_) => "Unwatch",
      Self::Terminate(_) => "Terminate",
    }
  }

  #[must_use]
  pub fn watch_message(&self) -> Option<&WatchMessage> {
    match self {
      Self::Watch(msg) => Some(msg),
      _ => None,
    }
  }

  #[must_use]
  pub fn unwatch_message(&self) -> Option<&UnwatchMessage> {
    match self {
      Self::Unwatch(msg) => Some(msg),
      _ => None,
    }
  }

  #[must_use]
  pub fn terminated_message(&self) -> Option<&TerminatedMessage> {
    match self {
      Self::Terminate(msg) => Some(msg),
      _ => None,
    }
  }
}

impl Message for SystemMessage {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other
      .as_any()
      .downcast_ref::<SystemMessage>()
      .map_or(false, |value| value == self)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    self.as_str().into()
  }
}

impl NotInfluenceReceiveTimeout for SystemMessage {}
