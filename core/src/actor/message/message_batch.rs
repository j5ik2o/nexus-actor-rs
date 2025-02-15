use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub struct MessageBatch {
  pub messages: Vec<Box<dyn Message>>,
}

impl Message for MessageBatch {
  fn as_any(&self) -> &dyn Any {
    self
  }
}
