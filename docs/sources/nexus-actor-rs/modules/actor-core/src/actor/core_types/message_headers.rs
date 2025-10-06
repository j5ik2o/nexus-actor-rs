#![cfg(feature = "alloc")]

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::Debug;

pub type HeaderMap = BTreeMap<String, String>;

pub trait ReadonlyMessageHeaders: Debug + Send + Sync + 'static {
  fn get(&self, key: &str) -> Option<String>;
  fn keys(&self) -> Vec<String>;
  fn length(&self) -> usize;
  fn to_map(&self) -> HeaderMap;
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

  pub fn get(&self, key: &str) -> Option<String> {
    self.0.get(key)
  }

  pub fn keys(&self) -> Vec<String> {
    self.0.keys()
  }

  pub fn length(&self) -> usize {
    self.0.length()
  }

  pub fn to_map(&self) -> HeaderMap {
    self.0.to_map()
  }
}

impl ReadonlyMessageHeaders for ReadonlyMessageHeadersHandle {
  fn get(&self, key: &str) -> Option<String> {
    self.0.get(key)
  }

  fn keys(&self) -> Vec<String> {
    self.0.keys()
  }

  fn length(&self) -> usize {
    self.0.length()
  }

  fn to_map(&self) -> HeaderMap {
    self.0.to_map()
  }
}

impl PartialEq for ReadonlyMessageHeadersHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ReadonlyMessageHeadersHandle {}
