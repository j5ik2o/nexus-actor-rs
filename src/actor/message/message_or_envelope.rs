use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use once_cell::sync::Lazy;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::message::message_handle::{Message, MessageHandle};
use crate::actor::message::system_message::SystemMessage;

#[derive(Debug, Default, Clone)]
pub struct MessageHeaders {
  inner: HashMap<String, String>,
}

impl PartialEq for MessageHeaders {
  fn eq(&self, other: &Self) -> bool {
    self.inner == other.inner
  }
}

impl Eq for MessageHeaders {}

impl MessageHeaders {
  pub fn new() -> Self {
    Self { inner: HashMap::new() }
  }

  pub fn with_values(values: HashMap<String, String>) -> Self {
    Self { inner: values }
  }

  pub fn set(&mut self, key: String, value: String) {
    self.inner.insert(key, value);
  }
}

pub static EMPTY_MESSAGE_HEADER: Lazy<Arc<MessageHeaders>> = Lazy::new(|| Arc::new(MessageHeaders::new()));

pub trait ReadonlyMessageHeaders: Debug + Send + Sync + 'static {
  fn get(&self, key: &str) -> Option<&String>;
  fn keys(&self) -> Vec<&String>;
  fn length(&self) -> usize;
  fn to_map(&self) -> HashMap<String, String>;
}

#[derive(Debug, Clone)]
pub struct ReadonlyMessageHeadersHandle(Arc<dyn ReadonlyMessageHeaders>);

impl ReadonlyMessageHeadersHandle {
  pub fn new_arc(header: Arc<dyn ReadonlyMessageHeaders>) -> Self {
    ReadonlyMessageHeadersHandle(header)
  }

  pub fn new(header: impl ReadonlyMessageHeaders + 'static) -> Self {
    ReadonlyMessageHeadersHandle(Arc::new(header))
  }
}

impl ReadonlyMessageHeaders for ReadonlyMessageHeadersHandle {
  fn get(&self, key: &str) -> Option<&String> {
    self.0.get(key)
  }

  fn keys(&self) -> Vec<&String> {
    self.0.keys()
  }

  fn length(&self) -> usize {
    self.0.length()
  }

  fn to_map(&self) -> HashMap<String, String> {
    self.0.to_map()
  }
}

impl ReadonlyMessageHeaders for MessageHeaders {
  fn get(&self, key: &str) -> Option<&String> {
    self.inner.get(key)
  }

  fn keys(&self) -> Vec<&String> {
    self.inner.keys().collect()
  }

  fn length(&self) -> usize {
    self.inner.len()
  }

  fn to_map(&self) -> HashMap<String, String> {
    self.inner.clone()
  }
}

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
    envelope
      .header
      .clone()
      .map(|h| MessageHeaders::with_values(h.inner.clone()))
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

// #[derive(Debug, Clone)]
// pub struct MessageOrEnvelope {
//   message: Option<MessageHandle>,
//   sender: Option<ExtendedPid>,
//   envelope: Option<MessageEnvelope>,
// }
//
// impl PartialEq for MessageOrEnvelope {
//   fn eq(&self, other: &Self) -> bool {
//     self.message == other.message && self.envelope == other.envelope && self.sender == other.sender
//   }
// }
//
// impl Message for MessageOrEnvelope {
//   fn eq_message(&self, other: &dyn Message) -> bool {
//     if let Some(other) = other.as_any().downcast_ref::<Self>() {
//       self.message == other.message && self.envelope == other.envelope && self.sender == other.sender
//     } else {
//       false
//     }
//   }
//
//   fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
//     self
//   }
// }
//
// impl MessageOrEnvelope {
//   pub fn of_message(message: MessageHandle) -> Self {
//     tracing::debug!(">>> MessageOrEnvelope::of_message: message = {:?}", message);
//     if message.as_any().downcast_ref::<MessageOrEnvelope>().is_some() {
//       panic!("MessageOrEnvelope can't be used as a message, {:?}", message);
//     }
//     if message.as_any().downcast_ref::<MessageHandle>().is_some() {
//       panic!("MessageOrEnvelope can't be used as a message, {:?}", message);
//     }
//     Self {
//       message: Some(message),
//       sender: None,
//       envelope: None,
//     }
//   }
//
//   pub fn of_envelope(envelope: MessageEnvelope) -> Self {
//     Self {
//       message: None,
//       sender: None,
//       envelope: Some(envelope),
//     }
//   }
//
//   pub fn with_sender(mut self, sender: Option<ExtendedPid>) -> Self {
//     self.sender = sender;
//     self
//   }
//
//   pub fn get_value(&self) -> MessageHandle {
//     match (self.message.clone(), self.envelope.clone()) {
//       (Some(msg), _) => msg,
//       (_, Some(env)) => env.message.clone(),
//       _ => panic!("MessageOrEnvelope is empty"),
//     }
//   }
//
//   pub fn get_message(&self) -> Option<MessageHandle> {
//     self.message.clone()
//   }
//
//   pub fn get_envelope(&self) -> Option<MessageEnvelope> {
//     self.envelope.clone()
//   }
//
//   pub fn get_sender(&self) -> Option<ExtendedPid> {
//     match (self.sender.clone(), self.envelope.clone()) {
//       (Some(sender), None) => Some(sender),
//       (None, Some(MessageEnvelope { sender, .. })) => sender.clone(),
//       _ => None, }
//   }
// }
