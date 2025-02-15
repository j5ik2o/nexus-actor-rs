use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub struct Touched {
  pub message: Box<dyn Message>,
}

impl Message for Touched {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
