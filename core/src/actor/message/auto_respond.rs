//! Auto respond message implementation.

use crate::actor::context::Context;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub struct AutoRespond {
  response: MessageHandle,
}

impl AutoRespond {
  pub fn new(response: MessageHandle) -> Self {
    Self { response }
  }

  pub async fn get_auto_response(&self, _context: &dyn Context) -> MessageHandle {
    self.response.clone()
  }
}

// Remove Message implementation to avoid conflict with blanket impl
