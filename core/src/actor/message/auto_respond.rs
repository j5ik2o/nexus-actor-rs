use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub struct AutoRespond {
  pub message: Box<dyn Message>,
}

impl AutoRespond {
  pub fn new(message: Box<dyn Message>) -> Self {
    Self { message }
  }
}

impl Message for AutoRespond {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
