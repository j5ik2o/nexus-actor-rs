use std::sync::Arc;
use std::time::Duration;

use crate::provider::{ClusterProvider, InMemoryClusterProvider};

/// Cluster の基本設定を保持する構造体。
#[derive(Debug, Clone)]
pub struct ClusterConfig {
  pub cluster_name: String,
  pub gossip_interval: Duration,
  pub actor_idle_timeout: Duration,
  pub request_timeout: Duration,
  pub provider: Arc<dyn ClusterProvider>,
}

impl ClusterConfig {
  pub fn new(cluster_name: impl Into<String>) -> Self {
    Self {
      cluster_name: cluster_name.into(),
      gossip_interval: Duration::from_secs(1),
      actor_idle_timeout: Duration::from_secs(30),
      request_timeout: Duration::from_secs(5),
      provider: Arc::new(InMemoryClusterProvider::default()),
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
}
