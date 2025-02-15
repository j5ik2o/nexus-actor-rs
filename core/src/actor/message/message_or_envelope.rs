use crate::actor::message::{Message, MessageHeaders};
use crate::actor::pid::Pid;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct MessageOrEnvelope {
  pub message: Box<dyn Message>,
  pub header: Option<MessageHeaders>,
  pub sender: Option<Pid>,
}

impl Message for MessageOrEnvelope {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

impl MessageOrEnvelope {
  pub fn new(message: Box<dyn Message>) -> Self {
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

  pub fn get_message(&self) -> &Box<dyn Message> {
    &self.message
  }

  pub fn into_message(self) -> Box<dyn Message> {
    self.message
  }

  pub fn get_sender(&self) -> Option<&Pid> {
    self.sender.as_ref()
  }

  pub fn get_header(&self) -> Option<&MessageHeaders> {
    self.header.as_ref()
  }
}
