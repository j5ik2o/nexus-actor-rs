use alloc::collections::BTreeMap;
use alloc::string::String;

/// Metadata associated with Failure. Used to hold remote/cluster layer information in the future.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureMetadata {
  /// Component name where the failure occurred
  pub component: Option<String>,
  /// Endpoint where the failure occurred
  pub endpoint: Option<String>,
  /// Transport used
  pub transport: Option<String>,
  /// Additional tag information
  pub tags: BTreeMap<String, String>,
}

impl FailureMetadata {
  /// Creates a new empty metadata.
  ///
  /// # Returns
  /// New `FailureMetadata` instance
  pub fn new() -> Self {
    Self {
      component: None,
      endpoint: None,
      transport: None,
      tags: BTreeMap::new(),
    }
  }

  /// Sets the component name.
  ///
  /// # Arguments
  /// * `component` - Component name
  ///
  /// # Returns
  /// `FailureMetadata` instance with component name set
  pub fn with_component(mut self, component: impl Into<String>) -> Self {
    self.component = Some(component.into());
    self
  }

  /// Sets the endpoint.
  ///
  /// # Arguments
  /// * `endpoint` - Endpoint
  ///
  /// # Returns
  /// `FailureMetadata` instance with endpoint set
  pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
    self.endpoint = Some(endpoint.into());
    self
  }

  /// Sets the transport.
  ///
  /// # Arguments
  /// * `transport` - Transport
  ///
  /// # Returns
  /// `FailureMetadata` instance with transport set
  pub fn with_transport(mut self, transport: impl Into<String>) -> Self {
    self.transport = Some(transport.into());
    self
  }

  /// Adds a tag.
  ///
  /// # Arguments
  /// * `key` - Tag key
  /// * `value` - Tag value
  ///
  /// # Returns
  /// `FailureMetadata` instance with tag added
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
