use async_trait::async_trait;
use std::collections::HashSet;
use std::fmt;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::kind::ClusterKind;

#[derive(Debug, Clone)]
pub struct ClusterProviderContext {
  pub cluster_name: String,
  pub kinds: Vec<String>,
}

impl ClusterProviderContext {
  pub fn new(cluster_name: impl Into<String>, kinds: impl IntoIterator<Item = String>) -> Self {
    Self {
      cluster_name: cluster_name.into(),
      kinds: kinds.into_iter().collect(),
    }
  }
}

#[derive(Debug, Error)]
pub enum ClusterProviderError {
  #[error("provider failed: {0}")]
  ProviderError(String),
}

#[async_trait]
pub trait ClusterProvider: Send + Sync + fmt::Debug + 'static {
  async fn start_member(&self, ctx: &ClusterProviderContext) -> Result<(), ClusterProviderError>;
  async fn start_client(&self, ctx: &ClusterProviderContext) -> Result<(), ClusterProviderError>;
  async fn shutdown(&self, graceful: bool) -> Result<(), ClusterProviderError>;
}

#[derive(Default)]
pub struct InMemoryClusterProvider {
  members: RwLock<HashSet<String>>,
  clients: RwLock<HashSet<String>>,
}

impl fmt::Debug for InMemoryClusterProvider {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("InMemoryClusterProvider")
      .field("members", &"…")
      .field("clients", &"…")
      .finish()
  }
}

impl InMemoryClusterProvider {
  pub fn new() -> Self {
    Self::default()
  }

  pub async fn members_snapshot(&self) -> Vec<String> {
    let guard = self.members.read().await;
    guard.iter().cloned().collect()
  }

  pub async fn clients_snapshot(&self) -> Vec<String> {
    let guard = self.clients.read().await;
    guard.iter().cloned().collect()
  }
}

#[async_trait]
impl ClusterProvider for InMemoryClusterProvider {
  async fn start_member(&self, ctx: &ClusterProviderContext) -> Result<(), ClusterProviderError> {
    let mut members = self.members.write().await;
    members.insert(ctx.cluster_name.clone());
    Ok(())
  }

  async fn start_client(&self, ctx: &ClusterProviderContext) -> Result<(), ClusterProviderError> {
    let mut clients = self.clients.write().await;
    clients.insert(ctx.cluster_name.clone());
    Ok(())
  }

  async fn shutdown(&self, _graceful: bool) -> Result<(), ClusterProviderError> {
    Ok(())
  }
}

pub fn provider_context_from_kinds(cluster_name: &str, kinds: &DashMap<String, ClusterKind>) -> ClusterProviderContext {
  let kind_names = kinds.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
  ClusterProviderContext::new(cluster_name.to_string(), kind_names)
}

use dashmap::DashMap;
