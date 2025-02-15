use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub enum SystemMessage {
  Started,
  Stopped,
  Restarting,
  Terminated,
  Watch,
  Unwatch,
  Failure,
}

impl Message for SystemMessage {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
