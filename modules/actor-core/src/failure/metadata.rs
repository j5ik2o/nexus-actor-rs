use alloc::collections::BTreeMap;
use alloc::string::String;

/// Failure に付随するメタデータ。将来 remote/cluster 層の情報を保持するために使用する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureMetadata {
  pub component: Option<String>,
  pub endpoint: Option<String>,
  pub transport: Option<String>,
  pub tags: BTreeMap<String, String>,
}

impl FailureMetadata {
  pub fn new() -> Self {
    Self {
      component: None,
      endpoint: None,
      transport: None,
      tags: BTreeMap::new(),
    }
  }

  pub fn with_component(mut self, component: impl Into<String>) -> Self {
    self.component = Some(component.into());
    self
  }

  pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
    self.endpoint = Some(endpoint.into());
    self
  }

  pub fn with_transport(mut self, transport: impl Into<String>) -> Self {
    self.transport = Some(transport.into());
    self
  }

  pub fn insert_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    self.tags.insert(key.into(), value.into());
    self
  }
}

impl Default for FailureMetadata {
  fn default() -> Self {
    Self::new()
  }
}
