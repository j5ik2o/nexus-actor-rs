use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Weak};
use thiserror::Error;
use tokio::sync::RwLock;

use crate::kind::ClusterKind;
use crate::partition::manager::{ClusterTopology, PartitionManager};
use crate::rendezvous::ClusterMember;

#[derive(Debug, Clone)]
pub struct ClusterProviderContext {
  pub cluster_name: String,
  pub node_address: String,
  pub kinds: Vec<String>,
  pub partition_manager: Arc<PartitionManager>,
}

impl ClusterProviderContext {
  pub fn new(
    cluster_name: impl Into<String>,
    node_address: impl Into<String>,
    kinds: impl IntoIterator<Item = String>,
    partition_manager: Arc<PartitionManager>,
  ) -> Self {
    Self {
      cluster_name: cluster_name.into(),
      node_address: node_address.into(),
      kinds: kinds.into_iter().collect(),
      partition_manager,
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

  async fn resolve_partition_manager(&self, _address: &str) -> Option<Arc<PartitionManager>>;
}

#[derive(Default)]
pub struct InMemoryClusterProvider {
  members: RwLock<HashMap<String, Vec<String>>>,
  clients: RwLock<HashSet<String>>,
  partition_managers: RwLock<HashMap<String, Weak<PartitionManager>>>,
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
    guard.keys().cloned().collect()
  }

  pub async fn topology_snapshot(&self) -> Vec<ClusterMember> {
    let guard = self.members.read().await;
    guard
      .iter()
      .map(|(address, kinds)| ClusterMember::new(address.clone(), kinds.clone()))
      .collect()
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
    members.insert(ctx.node_address.clone(), ctx.kinds.clone());
    drop(members);

    {
      let mut pms = self.partition_managers.write().await;
      pms.insert(ctx.node_address.clone(), Arc::downgrade(&ctx.partition_manager));
    }

    self.broadcast_topology().await;
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

  async fn resolve_partition_manager(&self, address: &str) -> Option<Arc<PartitionManager>> {
    let guard = self.partition_managers.read().await;
    guard.get(address).and_then(|weak| weak.upgrade())
  }
}

pub fn provider_context_from_kinds(
  cluster_name: &str,
  node_address: String,
  kinds: &DashMap<String, ClusterKind>,
  partition_manager: Arc<PartitionManager>,
) -> ClusterProviderContext {
  let kind_names = kinds.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
  ClusterProviderContext::new(cluster_name.to_string(), node_address, kind_names, partition_manager)
}

use dashmap::DashMap;

impl InMemoryClusterProvider {
  async fn broadcast_topology(&self) {
    let members = self.topology_snapshot().await;

    let managers = {
      let mut guard = self.partition_managers.write().await;
      let mut alive = Vec::new();
      let mut stale = Vec::new();
      for (address, weak) in guard.iter() {
        if let Some(manager) = weak.upgrade() {
          alive.push(manager);
        } else {
          stale.push(address.clone());
        }
      }
      for address in stale {
        guard.remove(&address);
      }
      alive
    };

    let topology = ClusterTopology { members };
    for manager in managers {
      manager.update_topology(topology.clone()).await;
    }
  }
}
