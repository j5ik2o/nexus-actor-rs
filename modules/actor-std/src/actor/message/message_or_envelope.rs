use crate::actor::core::ExtendedPid;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::message_headers::MessageHeaders;
use crate::actor::message::system_message::SystemMessage;
use crate::actor::message::Message;
use nexus_actor_core_rs::actor::core_types::message_envelope::CoreMessageEnvelope;
use nexus_actor_core_rs::actor::core_types::message_headers::ReadonlyMessageHeaders;
use nexus_message_derive_rs::Message as DeriveMessage;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, DeriveMessage)]
pub struct MessageEnvelope {
  inner: CoreMessageEnvelope,
}

impl MessageEnvelope {
  pub fn new(message_handle: MessageHandle) -> Self {
    if message_handle.as_any().is::<SystemMessage>() {
      tracing::warn!("SystemMessage can't be used as a message, {:?}", message_handle);
    }
    Self {
      inner: CoreMessageEnvelope::new(message_handle),
    }
  }

  pub fn from_core(inner: CoreMessageEnvelope) -> Self {
    Self { inner }
  }

  pub fn with_header(mut self, header: MessageHeaders) -> Self {
    self.inner = self.inner.with_header(header.to_map());
    self
  }

  pub fn with_sender(mut self, sender: ExtendedPid) -> Self {
    self.inner = self.inner.with_sender(sender.to_core());
    self
  }

  pub fn get_message_handle(&self) -> MessageHandle {
    self.inner.message_handle().clone()
  }

  pub fn get_sender(&self) -> Option<ExtendedPid> {
    self.inner.sender().map(|pid| ExtendedPid::from_core(pid.clone()))
  }

  pub fn get_header(&self) -> Option<MessageHeaders> {
    self
      .inner
      .header()
      .cloned()
      .map(|map| MessageHeaders::with_values(map.into_iter()))
  }

  pub fn get_header_value(&self, key: &str) -> Option<String> {
    self.inner.get(key)
  }

  pub fn set_header(&mut self, key: String, value: String) {
    self.inner.set_header_entry(key, value);
  }

  pub fn get_headers(&self) -> Option<MessageHeaders> {
    self.get_header()
  }

  pub fn into_core(self) -> CoreMessageEnvelope {
    self.inner
  }

  pub fn as_core(&self) -> &CoreMessageEnvelope {
    &self.inner
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
      envelope.get_header(),
      envelope.get_message_handle(),
      envelope.get_sender(),
    )
  } else {
    (None, message_handle, None)
  }
}

pub fn unwrap_envelope_header(message_handle: MessageHandle) -> Option<MessageHeaders> {
  if let Some(envelope) = message_handle.to_typed::<MessageEnvelope>() {
    envelope.get_header()
  } else {
    None
  }
}

pub fn unwrap_envelope_message(message_handle: MessageHandle) -> MessageHandle {
  if let Some(envelope) = message_handle.to_typed::<MessageEnvelope>() {
    envelope.get_message_handle()
  } else {
    message_handle
  }
}

pub fn unwrap_envelope_sender(message_handle: MessageHandle) -> Option<ExtendedPid> {
  if let Some(envelope) = message_handle.to_typed::<MessageEnvelope>() {
    envelope.get_sender()
  } else {
    None
  }
}

#[cfg(test)]
mod tests;
