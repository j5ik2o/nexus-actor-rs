use std::sync::Arc;
use std::time::Duration;

use crate::identity_lookup::{DistributedIdentityLookup, IdentityLookupHandle};
use crate::provider::{ClusterProvider, InMemoryClusterProvider};

#[derive(Debug, Clone)]
pub struct RemoteOptions {
  pub host: String,
  pub port: u16,
  pub advertised_address: Option<String>,
}

impl RemoteOptions {
  pub fn new(host: impl Into<String>, port: u16) -> Self {
    Self {
      host: host.into(),
      port,
      advertised_address: None,
    }
  }

  pub fn with_advertised_address(mut self, address: impl Into<String>) -> Self {
    self.advertised_address = Some(address.into());
    self
  }

  pub fn host(&self) -> &str {
    &self.host
  }

  pub fn port(&self) -> u16 {
    self.port
  }

  pub fn advertised_address(&self) -> Option<&str> {
    self.advertised_address.as_deref()
  }
}

/// Cluster の基本設定を保持する構造体。
#[derive(Debug, Clone)]
pub struct ClusterConfig {
  pub cluster_name: String,
  pub gossip_interval: Duration,
  pub actor_idle_timeout: Duration,
  pub request_timeout: Duration,
  pub provider: Arc<dyn ClusterProvider>,
  pub identity_lookup: IdentityLookupHandle,
  pub remote: Option<RemoteOptions>,
}

impl ClusterConfig {
  pub fn new(cluster_name: impl Into<String>) -> Self {
    Self {
      cluster_name: cluster_name.into(),
      gossip_interval: Duration::from_secs(1),
      actor_idle_timeout: Duration::from_secs(30),
      request_timeout: Duration::from_secs(5),
      provider: Arc::new(InMemoryClusterProvider::default()),
      identity_lookup: Arc::new(DistributedIdentityLookup::default()),
      remote: None,
    }
  }

  pub fn with_gossip_interval(mut self, interval: Duration) -> Self {
    self.gossip_interval = interval;
    self
  }

  pub fn with_actor_idle_timeout(mut self, timeout: Duration) -> Self {
    self.actor_idle_timeout = timeout;
    self
  }

  pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
    self.request_timeout = timeout;
    self
  }

  pub fn request_timeout(&self) -> Duration {
    self.request_timeout
  }

  pub fn with_provider(mut self, provider: Arc<dyn ClusterProvider>) -> Self {
    self.provider = provider;
    self
  }

  pub fn provider(&self) -> Arc<dyn ClusterProvider> {
    self.provider.clone()
  }

  pub fn with_identity_lookup(mut self, identity_lookup: IdentityLookupHandle) -> Self {
    self.identity_lookup = identity_lookup;
    self
  }

  pub fn identity_lookup(&self) -> IdentityLookupHandle {
    self.identity_lookup.clone()
  }

  pub fn with_remote_options(mut self, remote: RemoteOptions) -> Self {
    self.remote = Some(remote);
    self
  }

  pub fn remote_options(&self) -> Option<&RemoteOptions> {
    self.remote.as_ref()
  }
}
