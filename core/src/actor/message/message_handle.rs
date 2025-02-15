use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct MessageHandle {
  pub message: Box<dyn Message>,
}

impl Message for MessageHandle {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

impl PartialEq for MessageHandle {
  fn eq(&self, other: &Self) -> bool {
    self.message.eq_message(other.message.as_ref())
  }
}

impl MessageHandle {
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
