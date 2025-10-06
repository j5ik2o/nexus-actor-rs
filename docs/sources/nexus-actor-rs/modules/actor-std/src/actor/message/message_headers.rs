use crate::actor::message::readonly_message_headers::ReadonlyMessageHeaders;
use dashmap::DashMap;
use nexus_actor_core_rs::actor::core_types::message_headers::HeaderMap;
use once_cell::sync::Lazy;
use std::sync::Arc;
use std::vec::Vec;

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

  pub fn with_values<I>(values: I) -> Self
  where
    I: IntoIterator<Item = (String, String)>, {
    let map = DashMap::new();
    for (k, v) in values.into_iter() {
      map.insert(k, v);
    }
    Self { inner: Arc::new(map) }
  }

  pub fn set(&mut self, key: String, value: String) {
    self.inner.insert(key, value);
  }

  pub fn to_map(&self) -> HeaderMap {
    self
      .inner
      .iter()
      .map(|entry| (entry.key().clone(), entry.value().clone()))
      .collect()
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

  fn to_map(&self) -> HeaderMap {
    MessageHeaders::to_map(self)
  }
}

pub static EMPTY_MESSAGE_HEADER: Lazy<Arc<MessageHeaders>> = Lazy::new(|| Arc::new(MessageHeaders::new()));
