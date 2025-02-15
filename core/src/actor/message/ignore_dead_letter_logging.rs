use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub struct IgnoreDeadLetterLogging;

impl Message for IgnoreDeadLetterLogging {
  fn as_any(&self) -> &dyn Any {
    self
  }
}
