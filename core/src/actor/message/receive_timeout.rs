use std::any::Any;

use crate::actor::message::Message;

#[derive(Debug, Clone)]
pub struct ReceiveTimeout;

impl Message for ReceiveTimeout {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().is::<ReceiveTimeout>()
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn message_type(&self) -> &'static str {
    "ReceiveTimeout"
  }
}
