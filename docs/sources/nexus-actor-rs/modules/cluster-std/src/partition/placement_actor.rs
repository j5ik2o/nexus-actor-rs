use std::sync::Arc;

use dashmap::DashMap;
use nexus_actor_std_rs::actor::context::{BasePart, ContextHandle, MessagePart, SenderPart, SpawnerPart, StopperPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ExtendedPid};
use nexus_actor_std_rs::actor::message::{MessageHandle, ResponseHandle, TerminatedMessage};

use crate::cluster::Cluster;
use crate::identity::ClusterIdentity;
use crate::messages::ClusterInit;
use crate::partition::manager::ClusterTopology;
use crate::partition::messages::{ActivationRequest, ActivationResponse};
use crate::rendezvous::Rendezvous;
use crate::virtual_actor::VirtualActorContext;
use tracing::warn;

#[derive(Debug, Clone)]
struct ActivationEntry {
  identity: ClusterIdentity,
  pid: ExtendedPid,
}

#[derive(Debug)]
pub struct PlacementActor {
  cluster: Arc<Cluster>,
  activations: DashMap<String, ActivationEntry>,
}

impl PlacementActor {
  pub fn new(cluster: Arc<Cluster>) -> Self {
    Self {
      cluster,
      activations: DashMap::new(),
    }
  }

  async fn sync_local_identity_cache(&self, identity: ClusterIdentity, pid: ExtendedPid) {
    let kind = identity.kind().to_string();
    let id = identity.id().to_string();
    let lookup = self.cluster.identity_lookup();

    if let Err(err) = lookup.set(identity, pid).await {
      warn!(?err, %kind, %id, "failed to update identity lookup cache after activation");
    }
  }

  async fn handle_activation_request(
    &self,
    request: ActivationRequest,
    mut context: ContextHandle,
  ) -> Result<(), ActorError> {
    let key = request.identity.as_key();

    if let Some(existing) = self.activations.get(&key) {
      let pid = existing.value().pid.clone();
      drop(existing);

      self
        .sync_local_identity_cache(request.identity.clone(), pid.clone())
        .await;

      context
        .respond(ResponseHandle::new(ActivationResponse { pid: Some(pid) }))
        .await;
      return Ok(());
    }

    let kind = match self.cluster.kind(request.identity.kind()) {
      Some(kind) => kind,
      None => {
        context
          .respond(ResponseHandle::new(ActivationResponse { pid: None }))
          .await;
        return Ok(());
      }
    };

    let props = kind.props(&request.identity).await;
    let pid = context.spawn_prefix(props, request.identity.id()).await;

    context.watch(&pid).await;

    let init = ClusterInit::new(VirtualActorContext::new(
      request.identity.clone(),
      self.cluster.actor_system(),
      (*self.cluster).clone(),
    ));
    context.send(pid.clone(), MessageHandle::new(init)).await;

    self.activations.insert(
      key,
      ActivationEntry {
        identity: request.identity.clone(),
        pid: pid.clone(),
      },
    );

    self
      .sync_local_identity_cache(request.identity.clone(), pid.clone())
      .await;

    context
      .respond(ResponseHandle::new(ActivationResponse { pid: Some(pid) }))
      .await;
    Ok(())
  }

  async fn handle_cluster_topology(
    &self,
    topology: ClusterTopology,
    mut context: ContextHandle,
  ) -> Result<(), ActorError> {
    let address = self.cluster.actor_system().get_address().await;
    let rendezvous = Rendezvous::new();
    rendezvous.update_members(topology.members);

    let identity_lookup = self.cluster.identity_lookup();
    let partition_manager = self.cluster.partition_manager();

    let handoffs = self
      .activations
      .iter()
      .filter_map(|entry| {
        let activation = entry.value();
        let owner = rendezvous.owner_for_kind_identity(activation.identity.kind(), activation.identity.id());
        match owner.as_deref() {
          Some(owner_addr) if owner_addr == address => None,
          _ => Some((
            entry.key().clone(),
            activation.identity.clone(),
            activation.pid.clone(),
            owner,
          )),
        }
      })
      .collect::<Vec<_>>();

    for (key, identity, pid, owner) in handoffs {
      if let Err(err) = identity_lookup.remove(&identity).await {
        warn!(
          ?err,
          kind = identity.kind(),
          id = identity.id(),
          "failed to purge identity from lookup during rebalance"
        );
      }

      if let Some(owner_addr) = owner {
        if owner_addr != address {
          if let Err(err) = partition_manager.activate(identity.clone()).await {
            warn!(
              ?err,
              kind = identity.kind(),
              id = identity.id(),
              owner = owner_addr,
              "failed to activate identity on new owner during rebalance"
            );
          }
        }
      }

      self.activations.remove(&key);
      context.poison(&pid).await;
    }

    Ok(())
  }
}

#[async_trait::async_trait]
impl Actor for PlacementActor {
  async fn receive(&mut self, context: ContextHandle) -> Result<(), ActorError> {
    let message_handle = context
      .get_message_handle_opt()
      .await
      .ok_or_else(|| ActorError::of_receive_error("message missing".into()))?;

    if let Some(request) = message_handle.to_typed::<ActivationRequest>() {
      return self.handle_activation_request(request, context).await;
    }

    if let Some(topology) = message_handle.to_typed::<ClusterTopology>() {
      return self.handle_cluster_topology(topology, context).await;
    }

    Ok(())
  }

  async fn pre_stop(&mut self, mut context: ContextHandle) -> Result<(), ActorError> {
    let pids = self
      .activations
      .iter()
      .map(|entry| entry.value().pid.clone())
      .collect::<Vec<_>>();

    for pid in pids {
      context.poison(&pid).await;
    }

    Ok(())
  }

  async fn post_child_terminate(&mut self, _: ContextHandle, terminated: &TerminatedMessage) -> Result<(), ActorError> {
    if let Some(core_pid) = &terminated.who {
      let terminated_pid = ExtendedPid::from_core(core_pid.clone());
      let key = self.activations.iter().find_map(|entry| {
        if entry.value().pid == terminated_pid {
          Some(entry.key().clone())
        } else {
          None
        }
      });

      if let Some(key) = key {
        self.activations.remove(&key);
      }
    }

    Ok(())
  }
}
