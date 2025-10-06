use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{SenderPart, StopperPart};
use nexus_actor_std_rs::actor::core::ExtendedPid;
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use nexus_actor_std_rs::actor::process::future::ActorFutureError;
use nexus_actor_std_rs::generated::actor::Pid;
use nexus_remote_std_rs::{
  ActivationHandler, ActivationHandlerError, Config as RemoteConfig, ConfigOption as RemoteConfigOption, Remote,
  ResponseStatusCode, TransportEndpoint,
};
use std::time::Duration;

use crate::config::ClusterConfig;
use crate::identity::ClusterIdentity;
use crate::identity_lookup::{
  identity_lookup_context_from_kinds, DistributedIdentityLookup, IdentityLookupContext, IdentityLookupError,
  IdentityLookupHandle,
};
use crate::kind::ClusterKind;
use crate::partition::manager::{ClusterTopology, PartitionManager, PartitionManagerError};
use crate::provider::{
  provider_context_from_kinds, ClusterProvider, ClusterProviderContext, ClusterProviderError, TopologyEvent,
};
use crate::rendezvous::ClusterMember;
use nexus_actor_core_rs::runtime::{CoreJoinHandle, CoreTaskFuture};
use nexus_actor_std_rs::event_stream::Subscription;
use tokio::sync::{oneshot, Mutex};
use tracing::{error, info, warn};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClusterError {
  #[error("kind not registered: {0}")]
  KindNotRegistered(String),
  #[error("request timed out after {0:?}")]
  RequestTimeout(Duration),
  #[error("request failed: {0}")]
  RequestFailed(ActorFutureError),
  #[error("identity lookup error: {0}")]
  IdentityLookup(IdentityLookupError),
  #[error("provider error: {0}")]
  Provider(ClusterProviderError),
  #[error("partition manager error: {0}")]
  Partition(PartitionManagerError),
}

impl ClusterError {
  fn from_actor_future_error(error: ActorFutureError, timeout: Duration) -> Self {
    match error {
      ActorFutureError::TimeoutError => ClusterError::RequestTimeout(timeout),
      other => ClusterError::RequestFailed(other),
    }
  }
}

impl From<IdentityLookupError> for ClusterError {
  fn from(value: IdentityLookupError) -> Self {
    ClusterError::IdentityLookup(value)
  }
}

impl From<ClusterProviderError> for ClusterError {
  fn from(value: ClusterProviderError) -> Self {
    ClusterError::Provider(value)
  }
}

impl From<PartitionManagerError> for ClusterError {
  fn from(value: PartitionManagerError) -> Self {
    ClusterError::Partition(value)
  }
}

#[derive(Debug, Clone)]
struct ClusterActivationHandler {
  cluster: Cluster,
}

#[async_trait]
impl ActivationHandler for ClusterActivationHandler {
  async fn activate(&self, kind: &str, identity: &str) -> Result<Option<Pid>, ActivationHandlerError> {
    let mut identity_str = identity.to_string();
    if identity_str.is_empty() {
      identity_str = self.cluster.actor_system().get_process_registry().await.next_id();
    }

    let cluster_identity = ClusterIdentity::new(kind.to_string(), identity_str);
    let partition_manager = self.cluster.partition_manager();

    match partition_manager.activate_local(cluster_identity.clone()).await {
      Ok(Some(pid)) => Ok(Some(pid.inner_pid.clone())),
      Ok(None) => Ok(None),
      Err(PartitionManagerError::RequestFailed(ActorFutureError::TimeoutError)) => Err(ActivationHandlerError::new(
        ResponseStatusCode::Timeout,
        "activation timeout",
      )),
      Err(err) => Err(ActivationHandlerError::new(ResponseStatusCode::Error, err.to_string())),
    }
  }
}

/// Virtual Actor を管理する最小限の Cluster 実装。
#[derive(Clone)]
pub struct Cluster {
  actor_system: Arc<ActorSystem>,
  config: ClusterConfig,
  provider: Arc<dyn ClusterProvider>,
  identity_lookup: IdentityLookupHandle,
  kinds: Arc<DashMap<String, ClusterKind>>,
  partition_manager: Arc<PartitionManager>,
  remote: Arc<Mutex<Option<Arc<Remote>>>>,
  remote_task: Arc<Mutex<Option<Arc<dyn CoreJoinHandle>>>>,
  topology_subscription: Arc<Mutex<Option<Subscription>>>,
}

impl fmt::Debug for Cluster {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("Cluster")
      .field("cluster_name", &self.config.cluster_name)
      .finish_non_exhaustive()
  }
}

impl Cluster {
  pub fn new(actor_system: Arc<ActorSystem>, config: ClusterConfig) -> Self {
    let provider = config.provider();
    let identity_lookup = config.identity_lookup();
    let partition_manager = Arc::new(PartitionManager::new());
    let cluster = Self {
      actor_system,
      config,
      provider,
      identity_lookup,
      kinds: Arc::new(DashMap::new()),
      partition_manager,
      remote: Arc::new(Mutex::new(None)),
      remote_task: Arc::new(Mutex::new(None)),
      topology_subscription: Arc::new(Mutex::new(None)),
    };
    cluster
      .partition_manager
      .ensure_cluster_attached(Arc::new(cluster.clone()));
    cluster.attach_partition_manager_to_lookup();
    cluster
  }

  pub fn config(&self) -> &ClusterConfig {
    &self.config
  }

  pub fn provider(&self) -> Arc<dyn ClusterProvider> {
    self.provider.clone()
  }

  pub fn identity_lookup(&self) -> IdentityLookupHandle {
    self.identity_lookup.clone()
  }

  pub fn partition_manager(&self) -> Arc<PartitionManager> {
    self.partition_manager.clone()
  }

  fn request_timeout(&self) -> Duration {
    self.config.request_timeout()
  }

  pub fn register_kind(&self, kind: ClusterKind) {
    self.kinds.insert(kind.name().to_string(), kind);
    self.spawn_local_topology_refresh();
  }

  pub fn deregister_kind(&self, kind: &str) {
    self.kinds.remove(kind);
    self.spawn_local_topology_refresh();
  }

  pub fn kind(&self, kind: &str) -> Option<ClusterKind> {
    self.kinds.get(kind).map(|entry| entry.value().clone())
  }

  pub fn actor_system(&self) -> Arc<ActorSystem> {
    self.actor_system.clone()
  }

  async fn provider_context(&self) -> ClusterProviderContext {
    let address = self.actor_system.get_address().await;
    let event_stream = self.actor_system.get_event_stream().await;
    let core_runtime = self.actor_system.core_runtime();
    provider_context_from_kinds(
      &self.config.cluster_name,
      address,
      &self.kinds,
      self.partition_manager.clone(),
      event_stream,
      core_runtime,
    )
  }

  async fn ensure_topology_logging(&self) {
    let mut guard = self.topology_subscription.lock().await;
    if guard.is_some() {
      return;
    }

    let event_stream = self.actor_system.get_event_stream().await;
    let cluster_name = self.config.cluster_name.clone();
    let subscription = event_stream
      .subscribe(move |message| {
        let cluster = cluster_name.clone();
        async move {
          if let Some(event) = message.to_typed::<TopologyEvent>() {
            info!(
              cluster = %event.cluster_name,
              observer = %cluster,
              node = ?event.node_address,
              event_type = %event.event_type,
              member_count = event.members.len(),
              "topology event observed"
            );
          }
        }
      })
      .await;
    *guard = Some(subscription);
  }

  async fn clear_topology_logging(&self) {
    let mut guard = self.topology_subscription.lock().await;
    if let Some(subscription) = guard.take() {
      let event_stream = self.actor_system.get_event_stream().await;
      event_stream.unsubscribe(subscription).await;
    }
  }

  async fn ensure_remote(&self) -> Result<(), ClusterError> {
    let remote_options = match self.config.remote_options() {
      Some(options) => options.clone(),
      None => return Ok(()),
    };

    if self.remote.lock().await.is_some() {
      return Ok(());
    }

    let mut remote_config_options = vec![
      RemoteConfigOption::with_host(remote_options.host()),
      RemoteConfigOption::with_port(remote_options.port()),
      RemoteConfigOption::with_transport_endpoint(TransportEndpoint::new(format!(
        "{}:{}",
        remote_options.host(),
        remote_options.port()
      ))),
    ];

    if let Some(advertised) = remote_options.advertised_address() {
      remote_config_options.push(RemoteConfigOption::with_advertised_address(advertised));
    }

    let remote_config = RemoteConfig::from(remote_config_options).await;
    let remote = Remote::new(self.actor_system.as_ref().clone(), remote_config).await;

    remote
      .set_activation_handler(Arc::new(ClusterActivationHandler { cluster: self.clone() }))
      .await;

    let remote_arc = Arc::new(remote);
    let (started_tx, started_rx) = oneshot::channel();
    let remote_clone = remote_arc.clone();
    let spawner = self.actor_system.core_runtime().spawner();
    let future: CoreTaskFuture = Box::pin(async move {
      let start_result = remote_clone
        .start_with_callback(|| async {
          let _ = started_tx.send(());
        })
        .await;
      if let Err(err) = start_result {
        error!(?err, "Remote server terminated with error");
      }
    });

    let handle = match spawner.spawn(future) {
      Ok(handle) => handle,
      Err(err) => {
        error!(?err, "Failed to spawn remote server task");
        return Err(ClusterError::Provider(ClusterProviderError::ProviderError(
          "failed to spawn remote".into(),
        )));
      }
    };

    let _ = started_rx.await;

    {
      let mut guard = self.remote.lock().await;
      *guard = Some(remote_arc);
    }

    {
      let mut guard = self.remote_task.lock().await;
      *guard = Some(handle);
    }

    self.ensure_topology_logging().await;

    Ok(())
  }

  fn identity_lookup_context(&self) -> IdentityLookupContext {
    identity_lookup_context_from_kinds(&self.config.cluster_name, &self.kinds)
  }

  fn attach_partition_manager_to_lookup(&self) {
    if let Some(distributed) = self
      .identity_lookup
      .as_any()
      .downcast_ref::<DistributedIdentityLookup>()
    {
      distributed.attach_partition_manager(self.partition_manager.clone());
    }
  }

  pub async fn start_member(&self) -> Result<(), ClusterError> {
    self.partition_manager.ensure_cluster_attached(Arc::new(self.clone()));
    self.attach_partition_manager_to_lookup();
    self.ensure_remote().await?;
    let ctx = self.provider_context().await;
    self
      .identity_lookup
      .setup(&self.identity_lookup_context())
      .await
      .map_err(ClusterError::from)?;
    self.partition_manager.start().await.map_err(ClusterError::from)?;
    self.update_local_topology().await;
    self.provider.start_member(&ctx).await.map_err(ClusterError::from)
  }

  pub async fn start_client(&self) -> Result<(), ClusterError> {
    self.partition_manager.ensure_cluster_attached(Arc::new(self.clone()));
    self.attach_partition_manager_to_lookup();
    self.ensure_remote().await?;
    let ctx = self.provider_context().await;
    self
      .identity_lookup
      .setup(&self.identity_lookup_context())
      .await
      .map_err(ClusterError::from)?;
    self.partition_manager.start().await.map_err(ClusterError::from)?;
    self.update_local_topology().await;
    self.provider.start_client(&ctx).await.map_err(ClusterError::from)
  }

  async fn update_local_topology(&self) {
    let address = self.actor_system.get_address().await;
    let kinds = self.kinds.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
    let member = ClusterMember::new(address, kinds);
    self
      .partition_manager
      .update_topology(ClusterTopology { members: vec![member] })
      .await;
  }

  fn spawn_local_topology_refresh(&self) {
    let actor_system = self.actor_system.clone();
    let kinds = self.kinds.clone();
    let partition_manager = self.partition_manager.clone();
    let spawner = actor_system.core_runtime().spawner();
    let future: CoreTaskFuture = Box::pin(async move {
      let address = actor_system.get_address().await;
      let kind_names = kinds.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
      let member = ClusterMember::new(address, kind_names);
      partition_manager
        .update_topology(ClusterTopology { members: vec![member] })
        .await;
    });

    if let Err(err) = spawner.spawn(future) {
      warn!(?err, "failed to spawn local topology refresh task");
    }
  }

  pub async fn shutdown(&self, graceful: bool) -> Result<(), ClusterError> {
    let mut root = self.actor_system.get_root_context().await;
    let pids = self.identity_lookup.list().await.map_err(ClusterError::from)?;

    for pid in &pids {
      let future = root.stop_future_with_timeout(pid, self.request_timeout()).await;
      if let Err(err) = future.result().await {
        warn!(?err, pid = %pid, "failed to stop actor during shutdown");
      }
    }

    self.identity_lookup.shutdown().await.map_err(ClusterError::from)?;
    if let Some(remote) = {
      let mut guard = self.remote.lock().await;
      guard.take()
    } {
      if let Err(err) = remote.shutdown(graceful).await {
        warn!(?err, "Failed to shutdown remote gracefully");
      }
    }
    if let Some(handle) = {
      let mut guard = self.remote_task.lock().await;
      guard.take()
    } {
      handle.join().await;
    }
    self.clear_topology_logging().await;
    self.partition_manager.stop().await;
    self.provider.shutdown(graceful).await.map_err(ClusterError::from)
  }

  pub async fn get(&self, identity: ClusterIdentity) -> Result<ExtendedPid, ClusterError> {
    match self.identity_lookup.get(&identity).await {
      Ok(Some(existing)) => return Ok(existing),
      Ok(None) => {}
      Err(IdentityLookupError::Partition(PartitionManagerError::RequestFailed(err))) => {
        return Err(ClusterError::from_actor_future_error(err, self.request_timeout()))
      }
      Err(other) => return Err(ClusterError::from(other)),
    }

    self.partition_manager.ensure_cluster_attached(Arc::new(self.clone()));
    self.attach_partition_manager_to_lookup();
    let activation = self.partition_manager.activate(identity.clone()).await;
    let pid = match activation {
      Ok(Some(pid)) => pid,
      Ok(None) => return Err(ClusterError::KindNotRegistered(identity.kind().to_string())),
      Err(PartitionManagerError::RequestFailed(err)) => {
        return Err(ClusterError::from_actor_future_error(err, self.request_timeout()))
      }
      Err(other) => return Err(ClusterError::from(other)),
    };

    self
      .identity_lookup
      .set(identity.clone(), pid.clone())
      .await
      .map_err(ClusterError::from)?;
    Ok(pid)
  }

  pub async fn forget(&self, identity: &ClusterIdentity) {
    let _ = self.identity_lookup.remove(identity).await;
  }

  pub async fn request<M>(&self, identity: ClusterIdentity, message: M) -> Result<(), ClusterError>
  where
    M: Message + Send + Sync + 'static, {
    let pid = self.get(identity).await?;
    let mut root = self.actor_system.get_root_context().await;
    root.send(pid, MessageHandle::new(message)).await;
    Ok(())
  }

  pub async fn request_message<M>(&self, identity: ClusterIdentity, message: M) -> Result<MessageHandle, ClusterError>
  where
    M: Message + Send + Sync + 'static, {
    let timeout = self.request_timeout();
    self.request_message_with_timeout(identity, message, timeout).await
  }

  pub async fn request_message_with_timeout<M>(
    &self,
    identity: ClusterIdentity,
    message: M,
    timeout: Duration,
  ) -> Result<MessageHandle, ClusterError>
  where
    M: Message + Send + Sync + 'static, {
    let future = self.request_future(identity, message, timeout).await?;
    let response = future
      .result()
      .await
      .map_err(|error| ClusterError::from_actor_future_error(error, timeout))?;
    Ok(response)
  }

  pub async fn request_future<M>(
    &self,
    identity: ClusterIdentity,
    message: M,
    timeout: std::time::Duration,
  ) -> Result<nexus_actor_std_rs::actor::process::actor_future::ActorFuture, ClusterError>
  where
    M: Message + Send + Sync + 'static, {
    let pid = self.get(identity).await?;
    let root = self.actor_system.get_root_context().await;
    Ok(root.request_future(pid, MessageHandle::new(message), timeout).await)
  }
}

#[cfg(test)]
mod tests;
