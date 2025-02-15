use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct ResponseHandle {
  pub message: Box<dyn Message>,
}

impl Message for ResponseHandle {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

impl PartialEq for ResponseHandle {
  fn eq(&self, other: &Self) -> bool {
    self.message.eq_message(other.message.as_ref())
  }
}

impl ResponseHandle {
  pub fn new(message: Box<dyn Message>) -> Self {
    Self { message }
  }

  pub fn get_message(&self) -> &Box<dyn Message> {
    &self.message
  }

  pub fn into_message(self) -> Box<dyn Message> {
    self.message
  }
}
