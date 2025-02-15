//! Message handle implementation.

use nexus_actor_utils_rs::collections::Element;
use std::fmt::Debug;

use crate::actor::message::Message;

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
}

impl Element for MessageHandle {}
