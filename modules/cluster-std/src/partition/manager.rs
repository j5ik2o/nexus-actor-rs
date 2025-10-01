use std::sync::{Arc, RwLock as StdRwLock};
use std::time::Duration;

use nexus_actor_std_rs::actor::context::{SenderPart, SpawnerPart, StopperPart};
use nexus_actor_std_rs::actor::core::{ExtendedPid, Props, SpawnError};
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use nexus_actor_std_rs::actor::process::future::ActorFutureError;
use nexus_message_derive_rs::Message as MessageDerive;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::cluster::Cluster;
use crate::identity::ClusterIdentity;
use crate::identity_lookup::DistributedIdentityLookup;
use crate::partition::messages::{ActivationRequest, ActivationResponse};
use crate::partition::placement_actor::PlacementActor;
use crate::rendezvous::{ClusterMember, Rendezvous};
use nexus_actor_std_rs::generated::actor::Pid;
use tracing::warn;

const PARTITION_ACTIVATOR_NAME: &str = "partition-activator";

#[derive(Debug, Clone, PartialEq, Eq, MessageDerive)]
pub struct ClusterTopology {
  pub members: Vec<ClusterMember>,
}

#[derive(Debug, Error)]
pub enum PartitionManagerError {
  #[error("failed to spawn placement actor: {0}")]
  SpawnFailed(SpawnError),
  #[error("cluster not attached to partition manager")]
  ClusterNotAttached,
  #[error("placement actor unavailable")]
  PlacementActorMissing,
  #[error("activation request failed: {0}")]
  RequestFailed(ActorFutureError),
  #[error("activation response missing")]
  InvalidResponse,
}

#[derive(Debug)]
pub struct PartitionManager {
  cluster: StdRwLock<Option<Arc<Cluster>>>,
  rendezvous: Rendezvous,
  placement_actor: RwLock<Option<ExtendedPid>>,
}

impl PartitionManager {
  pub fn new() -> Self {
    Self {
      cluster: StdRwLock::new(None),
      rendezvous: Rendezvous::new(),
      placement_actor: RwLock::new(None),
    }
  }

  pub fn ensure_cluster_attached(&self, cluster: Arc<Cluster>) {
    let mut guard = self.cluster.write().unwrap();
    if guard.is_none() {
      *guard = Some(cluster);
    }
  }

  fn get_cluster(&self) -> Result<Arc<Cluster>, PartitionManagerError> {
    self
      .cluster
      .read()
      .unwrap()
      .clone()
      .ok_or(PartitionManagerError::ClusterNotAttached)
  }

  pub async fn start(&self) -> Result<(), PartitionManagerError> {
    if self.placement_actor.read().await.is_some() {
      return Ok(());
    }

    let cluster = self.get_cluster()?;
    let cluster_for_actor = cluster.clone();
    let props = Props::from_async_actor_producer(move |_| {
      let cluster = cluster_for_actor.clone();
      async move { PlacementActor::new(cluster) }
    })
    .await;

    let mut root = cluster.actor_system().get_root_context().await;
    match root.spawn_named(props, PARTITION_ACTIVATOR_NAME).await {
      Ok(pid) => {
        let mut guard = self.placement_actor.write().await;
        *guard = Some(pid);
        Ok(())
      }
      Err(err) => Err(PartitionManagerError::SpawnFailed(err)),
    }
  }

  pub async fn stop(&self) {
    let pid = {
      let mut guard = self.placement_actor.write().await;
      guard.take()
    };

    if let Some(pid) = pid {
      let cluster = match self.get_cluster() {
        Ok(cluster) => cluster,
        Err(_) => return,
      };
      let mut root = cluster.actor_system().get_root_context().await;
      let future = root.poison_future_with_timeout(&pid, self.request_timeout()).await;
      if let Err(err) = future.result().await {
        warn!(?err, actor = %pid, "failed to poison placement actor during shutdown");
      }
    }
  }

  pub async fn update_topology(&self, topology: ClusterTopology) {
    self.rendezvous.update_members(topology.members.clone());

    self.sync_identity_lookup().await;

    let pid = {
      let guard = self.placement_actor.read().await;
      guard.as_ref().cloned()
    };

    if let Some(pid) = pid {
      let Ok(cluster) = self.get_cluster() else {
        return;
      };
      let mut root = cluster.actor_system().get_root_context().await;
      root.send(pid, MessageHandle::new(topology)).await;
    }
  }

  async fn sync_identity_lookup(&self) {
    let Ok(cluster) = self.get_cluster() else {
      return;
    };

    let identity_lookup = cluster.identity_lookup();

    let Some(distributed) = identity_lookup.as_any().downcast_ref::<DistributedIdentityLookup>() else {
      return;
    };

    let manager = cluster.partition_manager();
    let local_address = cluster.actor_system().get_address().await;

    distributed.sync_after_topology_change(manager, local_address).await;
  }

  pub fn owner_for(&self, kind: &str, identity: &str) -> Option<String> {
    self.rendezvous.owner_for_kind_identity(kind, identity)
  }

  pub async fn placement_actor_pid(&self) -> Option<ExtendedPid> {
    let guard = self.placement_actor.read().await;
    guard.clone()
  }

  fn request_timeout(&self) -> Duration {
    self
      .get_cluster()
      .map(|cluster| cluster.config().request_timeout())
      .unwrap_or_else(|_| Duration::from_millis(0))
  }

  async fn ensure_local_placement_pid(&self) -> Result<ExtendedPid, PartitionManagerError> {
    self.start().await?;
    self
      .placement_actor_pid()
      .await
      .ok_or(PartitionManagerError::PlacementActorMissing)
  }

  pub async fn activate(&self, identity: ClusterIdentity) -> Result<Option<ExtendedPid>, PartitionManagerError> {
    self.start().await?;
    let cluster = self.get_cluster()?;
    let timeout = cluster.config().request_timeout();
    let owner = self.owner_for(identity.kind(), identity.id());
    let local_address = cluster.actor_system().get_address().await;

    if let Some(addr) = owner.clone() {
      if addr != local_address {
        if let Some(provider_manager) = cluster.provider().resolve_partition_manager(&addr).await {
          return provider_manager.activate_local(identity).await;
        }
        let target_pid = ExtendedPid::new(Pid::new(&addr, PARTITION_ACTIVATOR_NAME));
        return self.request_activation(cluster, identity, timeout, target_pid).await;
      }
    }

    self.activate_local(identity).await
  }

  pub(crate) async fn activate_local(
    &self,
    identity: ClusterIdentity,
  ) -> Result<Option<ExtendedPid>, PartitionManagerError> {
    let cluster = self.get_cluster()?;
    let timeout = cluster.config().request_timeout();
    let target_pid = self.ensure_local_placement_pid().await?;
    self.request_activation(cluster, identity, timeout, target_pid).await
  }

  async fn request_activation(
    &self,
    cluster: Arc<Cluster>,
    identity: ClusterIdentity,
    timeout: Duration,
    target_pid: ExtendedPid,
  ) -> Result<Option<ExtendedPid>, PartitionManagerError> {
    let root = cluster.actor_system().get_root_context().await;
    let future = root
      .request_future(
        target_pid,
        MessageHandle::new(ActivationRequest {
          identity: identity.clone(),
        }),
        timeout,
      )
      .await;

    let response = future.result().await.map_err(PartitionManagerError::RequestFailed)?;
    let activation = response
      .to_typed::<ActivationResponse>()
      .ok_or(PartitionManagerError::InvalidResponse)?;
    Ok(activation.pid)
  }
}
