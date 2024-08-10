use crate::actor::message::{Message, MessageHandle};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct MessageBatch(Vec<MessageHandle>);

impl MessageBatch {
  pub fn new(messages: impl IntoIterator<Item = MessageHandle>) -> Self {
    Self(messages.into_iter().collect::<Vec<_>>())
  }

  pub fn get_messages(&self) -> &Vec<MessageHandle> {
    &self.0
  }
}

impl Message for MessageBatch {
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other) = other.as_any().downcast_ref::<MessageBatch>() {
      self.0 == other.0
    } else {
      false
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
