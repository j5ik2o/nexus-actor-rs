use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;

use crate::actor::message::readonly_message_headers::ReadonlyMessageHeaders;

#[derive(Debug, Default, Clone)]
pub struct MessageHeaders {
  inner: Arc<DashMap<String, String>>,
}

impl PartialEq for MessageHeaders {
  fn eq(&self, other: &Self) -> bool {
    self.to_map() == other.to_map()
  }
}

impl Eq for MessageHeaders {}

impl MessageHeaders {
  pub fn new() -> Self {
    Self {
      inner: Arc::new(DashMap::new()),
    }
  }

  pub fn with_values(values: HashMap<String, String>) -> Self {
    Self {
      inner: Arc::new(DashMap::from_iter(values)),
    }
  }

  pub fn set(&mut self, key: String, value: String) {
    self.inner.insert(key, value);
  }

  pub fn to_map(&self) -> HashMap<String, String> {
    HashMap::from_iter((*self.inner).clone())
  }
}

impl ReadonlyMessageHeaders for MessageHeaders {
  fn get(&self, key: &str) -> Option<String> {
    self.inner.get(key).map(|e| e.value().clone())
  }

  fn keys(&self) -> Vec<String> {
    self.inner.iter().map(|e| e.key().clone()).collect()
  }

  fn length(&self) -> usize {
    self.inner.len()
  }

  fn to_map(&self) -> HashMap<String, String> {
    HashMap::from_iter((*self.inner).clone())
  }
}

pub static EMPTY_MESSAGE_HEADER: Lazy<Arc<MessageHeaders>> = Lazy::new(|| Arc::new(MessageHeaders::new()));
