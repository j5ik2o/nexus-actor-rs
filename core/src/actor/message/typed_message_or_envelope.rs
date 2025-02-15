use crate::actor::message::{Message, MessageHeaders};
use crate::actor::pid::Pid;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct TypedMessageOrEnvelope<T: Message> {
  pub message: T,
  pub header: Option<MessageHeaders>,
  pub sender: Option<Pid>,
}

impl<T: Message> Message for TypedMessageOrEnvelope<T> {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

impl<T: Message> TypedMessageOrEnvelope<T> {
  pub fn new(message: T) -> Self {
    Self {
      message,
      header: None,
      sender: None,
    }
  }

  pub fn with_header(mut self, header: MessageHeaders) -> Self {
    self.header = Some(header);
    self
  }

  pub fn with_sender(mut self, sender: Pid) -> Self {
    self.sender = Some(sender);
    self
  }

  pub fn get_message(&self) -> &T {
    &self.message
  }

  pub fn into_message(self) -> T {
    self.message
  }

  pub fn get_sender(&self) -> Option<&Pid> {
    self.sender.as_ref()
  }

  pub fn get_header(&self) -> Option<&MessageHeaders> {
    self.header.as_ref()
  }
}
