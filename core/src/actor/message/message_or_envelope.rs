//! Message or envelope implementation.

use std::any::Any;
use std::fmt::Debug;

use crate::actor::message::{Message, MessageHeaders};
use crate::actor::pid::Pid;

#[derive(Debug, Clone)]
pub enum MessageOrEnvelope {
  Message(Box<dyn Message>),
  Envelope {
    message: Box<dyn Message>,
    header: MessageHeaders,
    sender: Option<Pid>,
  },
}

impl Message for MessageOrEnvelope {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

impl MessageOrEnvelope {
  pub fn new_message(message: Box<dyn Message>) -> Self {
    Self::Message(message)
  }

  pub fn new_envelope(message: Box<dyn Message>, header: MessageHeaders, sender: Option<Pid>) -> Self {
    Self::Envelope {
      message,
      header,
      sender,
    }
  }

  pub fn get_message(&self) -> &Box<dyn Message> {
    match self {
      Self::Message(msg) => msg,
      Self::Envelope { message, .. } => message,
    }
  }

  pub fn get_header(&self) -> Option<&MessageHeaders> {
    match self {
      Self::Message(_) => None,
      Self::Envelope { header, .. } => Some(header),
    }
  }

  pub fn get_sender(&self) -> Option<&Pid> {
    match self {
      Self::Message(_) => None,
      Self::Envelope { sender, .. } => sender.as_ref(),
    }
  }
}
