use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

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
