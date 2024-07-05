use std::any::Any;
use std::fmt::Debug;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::message::message::Message;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::message_headers::MessageHeaders;
use crate::actor::message::readonly_message_headers::ReadonlyMessageHeaders;
use crate::actor::message::system_message::SystemMessage;

#[derive(Debug, Clone)]
pub struct MessageEnvelope {
  header: Option<MessageHeaders>,
  message: MessageHandle,
  sender: Option<ExtendedPid>,
}

impl PartialEq for MessageEnvelope {
  fn eq(&self, other: &Self) -> bool {
    self.header == other.header && self.message == other.message && self.sender == other.sender
  }
}

impl Message for MessageEnvelope {
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other) = other.as_any().downcast_ref::<Self>() {
      self.header == other.header && self.message == other.message && self.sender == other.sender
    } else {
      false
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

impl MessageEnvelope {
  pub fn new(message: MessageHandle) -> Self {
    if message.as_any().is::<SystemMessage>() {
      tracing::warn!("SystemMessage can't be used as a message, {:?}", message);
    }
    Self {
      header: None,
      message,
      sender: None,
    }
  }

  pub fn with_header(mut self, header: MessageHeaders) -> Self {
    self.header = Some(header);
    self
  }

  pub fn with_sender(mut self, sender: ExtendedPid) -> Self {
    self.sender = Some(sender);
    self
  }

  pub fn get_message(&self) -> MessageHandle {
    self.message.clone()
  }

  pub fn get_sender(&self) -> Option<ExtendedPid> {
    self.sender.clone()
  }

  pub fn get_header(&self) -> Option<MessageHeaders> {
    self.header.clone()
  }

  pub fn get_header_value(&self, key: &str) -> Option<String> {
    self.header.as_ref().and_then(|h| h.get(key).cloned())
  }

  pub fn set_header(&mut self, key: String, value: String) {
    if self.header.is_none() {
      self.header = Some(MessageHeaders::default());
    }
    if let Some(h) = &mut self.header {
      h.set(key, value);
    }
  }

  pub fn get_headers(&self) -> Option<MessageHeaders> {
    self.header.clone()
  }
}

pub fn wrap_envelope(message: MessageHandle) -> MessageEnvelope {
  if let Some(envelope) = message.as_any().downcast_ref::<MessageEnvelope>() {
    envelope.clone()
  } else {
    MessageEnvelope::new(message)
  }
}

pub fn unwrap_envelope(message: MessageHandle) -> (Option<MessageHeaders>, MessageHandle, Option<ExtendedPid>) {
  if let Some(envelope) = message.as_any().downcast_ref::<MessageEnvelope>() {
    (
      envelope.header.clone(),
      envelope.message.clone(),
      envelope.sender.clone(),
    )
  } else {
    (None, message, None)
  }
}

pub fn unwrap_envelope_header(message: MessageHandle) -> Option<MessageHeaders> {
  if let Some(envelope) = message.as_any().downcast_ref::<MessageEnvelope>() {
    envelope.header.clone().map(|h| MessageHeaders::with_values(h.to_map()))
  } else {
    None
  }
}

pub fn unwrap_envelope_message(message: MessageHandle) -> MessageHandle {
  if let Some(envelope) = message.as_any().downcast_ref::<MessageEnvelope>() {
    envelope.message.clone()
  } else {
    message
  }
}

pub fn unwrap_envelope_sender(message: MessageHandle) -> Option<ExtendedPid> {
  if let Some(envelope) = message.as_any().downcast_ref::<MessageEnvelope>() {
    envelope.sender.clone()
  } else {
    None
  }
}
