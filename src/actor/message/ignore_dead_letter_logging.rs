use std::any::Any;

use crate::actor::message::message_handle::Message;

#[derive(Debug, Clone)]
pub struct IgnoreDeadLetterLogging {}

impl Message for IgnoreDeadLetterLogging {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().is::<IgnoreDeadLetterLogging>()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
