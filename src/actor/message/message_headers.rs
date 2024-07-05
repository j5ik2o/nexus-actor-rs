use crate::actor::message::readonly_message_headers::ReadonlyMessageHeaders;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;

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

  pub fn to_map(&self) -> HashMap<String, String> {
    self.inner.clone()
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

pub static EMPTY_MESSAGE_HEADER: Lazy<Arc<MessageHeaders>> = Lazy::new(|| Arc::new(MessageHeaders::new()));
