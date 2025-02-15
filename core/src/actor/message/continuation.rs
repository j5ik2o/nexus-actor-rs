use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub struct Continuation {
  pub message: Box<dyn Message>,
}

impl Message for Continuation {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
