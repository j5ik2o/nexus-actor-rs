//! Message handle implementation.

use crate::actor::message::Message;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct MessageHandle {
  message: Box<dyn Message>,
}

impl MessageHandle {
  pub fn new(message: Box<dyn Message>) -> Self {
    Self { message }
  }

  pub fn get_message(&self) -> &Box<dyn Message> {
    &self.message
  }

  pub async fn to_typed<T: Message + Clone>(&self) -> Option<T> {
    self.message.as_any().downcast_ref::<T>().cloned()
  }
}
