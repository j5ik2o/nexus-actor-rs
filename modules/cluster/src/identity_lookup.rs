use std::any::Any;
use std::fmt;
use std::sync::{Arc, RwLock as StdRwLock};

use async_trait::async_trait;
use dashmap::DashMap;
use thiserror::Error;
use tracing::warn;

use nexus_actor_std_rs::actor::core::ExtendedPid;

use crate::identity::ClusterIdentity;
use crate::kind::ClusterKind;
use crate::partition::manager::{PartitionManager, PartitionManagerError};

#[derive(Debug, Clone)]
pub struct IdentityLookupContext {
  pub cluster_name: String,
  pub kinds: Vec<String>,
}

impl IdentityLookupContext {
  pub fn new(cluster_name: impl Into<String>, kinds: impl IntoIterator<Item = String>) -> Self {
    Self {
      cluster_name: cluster_name.into(),
      kinds: kinds.into_iter().collect(),
    }
  }
}

#[derive(Debug, Error)]
pub enum IdentityLookupError {
  #[error("identity lookup operation failed: {0}")]
  OperationFailed(String),
  #[error("identity lookup partition error: {0}")]
  Partition(PartitionManagerError),
}

#[async_trait]
pub trait IdentityLookup: Send + Sync + fmt::Debug + 'static {
  async fn setup(&self, _ctx: &IdentityLookupContext) -> Result<(), IdentityLookupError> {
    Ok(())
  }

  async fn shutdown(&self) -> Result<(), IdentityLookupError> {
    Ok(())
  }

  async fn get(&self, identity: &ClusterIdentity) -> Result<Option<ExtendedPid>, IdentityLookupError>;

  async fn set(&self, identity: ClusterIdentity, pid: ExtendedPid) -> Result<(), IdentityLookupError>;

  async fn remove(&self, identity: &ClusterIdentity) -> Result<(), IdentityLookupError>;

  async fn list(&self) -> Result<Vec<ExtendedPid>, IdentityLookupError> {
    Ok(Vec::new())
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync);
}

#[derive(Default)]
pub struct InMemoryIdentityLookup {
  entries: DashMap<ClusterIdentity, ExtendedPid>,
}

impl fmt::Debug for InMemoryIdentityLookup {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("InMemoryIdentityLookup").finish()
  }
}

impl InMemoryIdentityLookup {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn snapshot(&self) -> Vec<(ClusterIdentity, ExtendedPid)> {
    self
      .entries
      .iter()
      .map(|entry| (entry.key().clone(), entry.value().clone()))
      .collect()
  }
}

#[async_trait]
impl IdentityLookup for InMemoryIdentityLookup {
  async fn get(&self, identity: &ClusterIdentity) -> Result<Option<ExtendedPid>, IdentityLookupError> {
    Ok(self.entries.get(identity).map(|entry| entry.value().clone()))
  }

  async fn set(&self, identity: ClusterIdentity, pid: ExtendedPid) -> Result<(), IdentityLookupError> {
    self.entries.insert(identity, pid);
    Ok(())
  }

  async fn remove(&self, identity: &ClusterIdentity) -> Result<(), IdentityLookupError> {
    self.entries.remove(identity);
    Ok(())
  }

  async fn list(&self) -> Result<Vec<ExtendedPid>, IdentityLookupError> {
    Ok(self.entries.iter().map(|entry| entry.value().clone()).collect())
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

pub fn identity_lookup_context_from_kinds(
  cluster_name: &str,
  kinds: &DashMap<String, ClusterKind>,
) -> IdentityLookupContext {
  let kind_names = kinds.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
  IdentityLookupContext::new(cluster_name.to_string(), kind_names)
}

pub type IdentityLookupHandle = Arc<dyn IdentityLookup>;

pub struct DistributedIdentityLookup {
  partition_manager: StdRwLock<Option<Arc<PartitionManager>>>,
  entries: DashMap<ClusterIdentity, ExtendedPid>,
}

impl DistributedIdentityLookup {
  pub fn new() -> Self {
    Self {
      partition_manager: StdRwLock::new(None),
      entries: DashMap::new(),
    }
  }

  pub fn attach_partition_manager(&self, manager: Arc<PartitionManager>) {
    let mut guard = self.partition_manager.write().unwrap();
    if guard.is_none() {
      *guard = Some(manager);
    }
  }

  fn partition_manager(&self) -> Result<Arc<PartitionManager>, IdentityLookupError> {
    self
      .partition_manager
      .read()
      .unwrap()
      .clone()
      .ok_or_else(|| IdentityLookupError::OperationFailed("partition manager not attached".into()))
  }

  fn record(&self, identity: ClusterIdentity, pid: ExtendedPid) {
    self.entries.insert(identity, pid);
  }

  fn forget(&self, identity: &ClusterIdentity) {
    self.entries.remove(identity);
  }

  pub fn snapshot(&self) -> Vec<(ClusterIdentity, ExtendedPid)> {
    self
      .entries
      .iter()
      .map(|entry| (entry.key().clone(), entry.value().clone()))
      .collect()
  }

  pub async fn sync_after_topology_change(&self, manager: Arc<PartitionManager>, local_address: String) {
    let mut to_remove = Vec::new();
    let mut to_reactivate = Vec::new();

    for entry in self.entries.iter() {
      let identity = entry.key().clone();
      let pid = entry.value().clone();
      match manager.owner_for(identity.kind(), identity.id()) {
        Some(owner) if owner == pid.address() => {}
        Some(owner) if owner == local_address => {
          to_reactivate.push(identity);
        }
        _ => {
          to_remove.push(identity);
        }
      }
    }

    for identity in to_remove {
      self.entries.remove(&identity);
    }

    for identity in to_reactivate {
      self.entries.remove(&identity);
      let kind = identity.kind().to_string();
      let id = identity.id().to_string();
      match manager.activate(identity.clone()).await {
        Ok(Some(pid)) => {
          self.record(identity, pid);
        }
        Ok(None) => {}
        Err(err) => {
          warn!(?err, %kind, %id, "failed to reactivate identity after topology change");
        }
      }
    }
  }
}

impl Default for DistributedIdentityLookup {
  fn default() -> Self {
    Self::new()
  }
}

impl fmt::Debug for DistributedIdentityLookup {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("DistributedIdentityLookup").finish()
  }
}

#[async_trait]
impl IdentityLookup for DistributedIdentityLookup {
  async fn get(&self, identity: &ClusterIdentity) -> Result<Option<ExtendedPid>, IdentityLookupError> {
    if let Some(existing) = self.entries.get(identity) {
      return Ok(Some(existing.value().clone()));
    }

    let manager = self.partition_manager()?;
    let pid = manager
      .activate(identity.clone())
      .await
      .map_err(IdentityLookupError::Partition)?;
    if let Some(pid) = pid {
      self.record(identity.clone(), pid.clone());
      Ok(Some(pid))
    } else {
      Ok(None)
    }
  }

  async fn set(&self, identity: ClusterIdentity, pid: ExtendedPid) -> Result<(), IdentityLookupError> {
    self.record(identity, pid);
    Ok(())
  }

  async fn remove(&self, identity: &ClusterIdentity) -> Result<(), IdentityLookupError> {
    self.forget(identity);
    Ok(())
  }

  async fn list(&self) -> Result<Vec<ExtendedPid>, IdentityLookupError> {
    Ok(self.entries.iter().map(|entry| entry.value().clone()).collect())
  }

  async fn shutdown(&self) -> Result<(), IdentityLookupError> {
    let manager = {
      let guard = self.partition_manager.read().unwrap();
      guard.clone()
    };
    if let Some(manager) = manager {
      manager.stop().await;
    }
    Ok(())
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
