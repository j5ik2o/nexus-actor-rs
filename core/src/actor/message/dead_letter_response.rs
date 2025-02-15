//! Dead letter response message implementation.

use crate::actor::message::Message;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub struct DeadLetterResponse {
  pub message: Box<dyn Message>,
}

impl Message for DeadLetterResponse {
  fn as_any(&self) -> &dyn std::any::Any {
    self
  }
}
