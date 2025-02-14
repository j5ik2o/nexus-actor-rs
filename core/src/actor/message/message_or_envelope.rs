use crate::actor::actor::ExtendedPid;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::message_headers::MessageHeaders;
use crate::actor::message::system_message::SystemMessage;
use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;
use tracing;

#[derive(Debug, Clone, PartialEq)]
pub struct MessageEnvelope {
  pub(crate) message_handle: MessageHandle,
  pub(crate) header: Option<MessageHeaders>,
  pub(crate) sender: Option<ExtendedPid>,
}

impl Message for MessageEnvelope {
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other) = other.as_any().downcast_ref::<MessageEnvelope>() {
      self.message_handle.eq_message(&other.message_handle)
    } else {
      false
    }
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn message_type(&self) -> &'static str {
    "MessageEnvelope"
  }
}

impl MessageEnvelope {
  pub fn new(message_handle: MessageHandle) -> Self {
    if message_handle.as_any().is::<SystemMessage>() {
      tracing::warn!("SystemMessage can't be used as a message, {:?}", message_handle);
    }
    Self {
      message_handle,
      header: None,
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

  pub fn get_message_handle(&self) -> MessageHandle {
    self.message_handle.clone()
  }

  pub fn get_sender(&self) -> Option<ExtendedPid> {
    self.sender.clone()
  }

  pub fn get_header(&self) -> Option<MessageHeaders> {
    self.header.clone()
  }

  pub fn get_header_value(&self, key: &str) -> Option<String> {
    self.header.as_ref().and_then(|h| h.get(key))
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

pub fn wrap_envelope(message_handle: MessageHandle) -> MessageEnvelope {
  if let Some(envelope) = message_handle.to_typed::<MessageEnvelope>() {
    envelope.clone()
  } else {
    MessageEnvelope::new(message_handle)
  }
}

pub fn unwrap_envelope(message_handle: MessageHandle) -> (Option<MessageHeaders>, MessageHandle, Option<ExtendedPid>) {
  if let Some(envelope) = message_handle.to_typed::<MessageEnvelope>() {
    (
      envelope.header.clone(),
      envelope.message_handle.clone(),
      envelope.sender.clone(),
    )
  } else {
    (None, message_handle, None)
  }
}

pub fn unwrap_envelope_header(message_handle: MessageHandle) -> Option<MessageHeaders> {
  if let Some(envelope) = message_handle.to_typed::<MessageEnvelope>() {
    envelope.header.clone()
  } else {
    None
  }
}

pub fn unwrap_envelope_message(message_handle: MessageHandle) -> MessageHandle {
  if let Some(envelope) = message_handle.to_typed::<MessageEnvelope>() {
    envelope.message_handle.clone()
  } else {
    message_handle
  }
}

pub fn unwrap_envelope_sender(message_handle: MessageHandle) -> Option<ExtendedPid> {
  if let Some(envelope) = message_handle.to_typed::<MessageEnvelope>() {
    envelope.sender.clone()
  } else {
    None
  }
}
