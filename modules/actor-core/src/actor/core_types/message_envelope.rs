#![cfg(feature = "alloc")]

use alloc::string::String;
use alloc::vec::Vec;

use super::message_handle::MessageHandle;
use super::message_headers::{HeaderMap, ReadonlyMessageHeaders};
use super::pid::CorePid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreMessageEnvelope {
  header: Option<HeaderMap>,
  message_handle: MessageHandle,
  sender: Option<CorePid>,
}

impl CoreMessageEnvelope {
  #[must_use]
  pub fn new(message_handle: MessageHandle) -> Self {
    Self {
      header: None,
      message_handle,
      sender: None,
    }
  }

  #[must_use]
  pub fn with_header(mut self, header: HeaderMap) -> Self {
    self.header = Some(header);
    self
  }

  #[must_use]
  pub fn with_sender(mut self, sender: CorePid) -> Self {
    self.sender = Some(sender);
    self
  }

  pub fn header(&self) -> Option<&HeaderMap> {
    self.header.as_ref()
  }

  pub fn sender(&self) -> Option<&CorePid> {
    self.sender.as_ref()
  }

  pub fn message_handle(&self) -> &MessageHandle {
    &self.message_handle
  }

  pub fn message_handle_mut(&mut self) -> &mut MessageHandle {
    &mut self.message_handle
  }

  pub fn take_header(&mut self) -> Option<HeaderMap> {
    self.header.take()
  }

  pub fn set_header_entry(&mut self, key: String, value: String) {
    let mut header = self.header.take().unwrap_or_default();
    header.insert(key, value);
    self.header = Some(header);
  }

  pub fn remove_header(&mut self, key: &str) {
    if let Some(mut header) = self.header.take() {
      header.remove(key);
      if header.is_empty() {
        self.header = None;
      } else {
        self.header = Some(header);
      }
    }
  }

  pub fn header_keys(&self) -> Option<Vec<String>> {
    self.header.as_ref().map(|h| h.keys().cloned().collect())
  }
}

impl ReadonlyMessageHeaders for CoreMessageEnvelope {
  fn get(&self, key: &str) -> Option<String> {
    self.header.as_ref().and_then(|h| h.get(key).cloned())
  }

  fn keys(&self) -> Vec<String> {
    self.header_keys().unwrap_or_default()
  }

  fn length(&self) -> usize {
    self.header.as_ref().map_or(0, |h| h.len())
  }

  fn to_map(&self) -> HeaderMap {
    self.header.clone().unwrap_or_default()
  }
}
