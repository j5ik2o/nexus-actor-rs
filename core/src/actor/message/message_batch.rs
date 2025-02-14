use crate::actor::message::{Message, MessageHandle};
use nexus_actor_message_derive_rs::Message;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageBatch(Vec<MessageHandle>);

impl Message for MessageBatch {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().is::<MessageBatch>()
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn message_type(&self) -> &'static str {
    "MessageBatch"
  }
}

impl MessageBatch {
  pub fn new(messages: impl IntoIterator<Item = MessageHandle>) -> Self {
    Self(messages.into_iter().collect::<Vec<_>>())
  }

  pub fn get_messages(&self) -> &Vec<MessageHandle> {
    &self.0
  }
}
