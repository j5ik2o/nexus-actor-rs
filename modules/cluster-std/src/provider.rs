use async_trait::async_trait;
use futures::StreamExt;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Weak};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::BroadcastStream;

use crate::generated::registry::cluster_registry_client::ClusterRegistryClient;
use crate::generated::registry::{
  HeartbeatRequest, JoinRequest, LeaveRequest, Member as ProtoMember, RegisterClientRequest, WatchUpdate,
};
use crate::kind::ClusterKind;
use crate::partition::manager::{ClusterTopology, PartitionManager};
use crate::rendezvous::ClusterMember;
use nexus_actor_core_rs::runtime::{CoreJoinHandle, CoreRuntime, CoreTaskFuture};
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use nexus_actor_std_rs::event_stream::EventStream;
use tonic::transport::{Channel, Endpoint};
use tonic::Status;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct ClusterProviderContext {
  pub cluster_name: String,
  pub node_address: String,
  pub kinds: Vec<String>,
  pub partition_manager: Arc<PartitionManager>,
  event_stream: Arc<EventStream>,
  core_runtime: CoreRuntime,
}

impl ClusterProviderContext {
  pub fn new(
    cluster_name: impl Into<String>,
    node_address: impl Into<String>,
    kinds: impl IntoIterator<Item = String>,
    partition_manager: Arc<PartitionManager>,
    event_stream: Arc<EventStream>,
    core_runtime: CoreRuntime,
  ) -> Self {
    Self {
      cluster_name: cluster_name.into(),
      node_address: node_address.into(),
      kinds: kinds.into_iter().collect(),
      partition_manager,
      event_stream,
      core_runtime,
    }
  }

  pub async fn publish_topology_event(&self, event: TopologyEvent) {
    self.event_stream.publish(MessageHandle::new(event)).await;
  }

  pub fn event_stream(&self) -> Arc<EventStream> {
    self.event_stream.clone()
  }

  pub fn core_runtime(&self) -> CoreRuntime {
    self.core_runtime.clone()
  }
}

#[derive(Debug, Error)]
pub enum ClusterProviderError {
  #[error("provider failed: {0}")]
  ProviderError(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct TopologyEvent {
  pub cluster_name: String,
  pub node_address: Option<String>,
  pub event_type: String,
  pub members: Vec<ClusterMember>,
}

impl TopologyEvent {
  pub fn registered(cluster_name: &str, node_address: &str, kinds: &[String]) -> Self {
    info!(
      cluster = cluster_name,
      node = node_address,
      kinds = ?kinds,
      "cluster node registered"
    );
    Self {
      cluster_name: cluster_name.to_string(),
      node_address: Some(node_address.to_string()),
      event_type: "registered".to_string(),
      members: Vec::new(),
    }
  }

  pub fn deregistered(cluster_name: &str, node_address: &str) -> Self {
    info!(cluster = cluster_name, node = node_address, "cluster node deregistered");
    Self {
      cluster_name: cluster_name.to_string(),
      node_address: Some(node_address.to_string()),
      event_type: "deregistered".to_string(),
      members: Vec::new(),
    }
  }

  pub fn snapshot(cluster_name: &str, members: Vec<ClusterMember>) -> Self {
    debug!(
      cluster = cluster_name,
      member_count = members.len(),
      "cluster topology snapshot"
    );
    Self {
      cluster_name: cluster_name.to_string(),
      node_address: None,
      event_type: "snapshot".to_string(),
      members,
    }
  }
}

impl Message for TopologyEvent {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other
      .as_any()
      .downcast_ref::<TopologyEvent>()
      .map_or(false, |event| event == self)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name::<Self>().to_string()
  }
}

#[async_trait]
pub trait ClusterProvider: Send + Sync + fmt::Debug + 'static {
  async fn start_member(&self, ctx: &ClusterProviderContext) -> Result<(), ClusterProviderError>;
  async fn start_client(&self, ctx: &ClusterProviderContext) -> Result<(), ClusterProviderError>;
  async fn shutdown(&self, graceful: bool) -> Result<(), ClusterProviderError>;

  async fn resolve_partition_manager(&self, _address: &str) -> Option<Arc<PartitionManager>>;
}

#[async_trait]
pub trait RegistryClient: Send + Sync + fmt::Debug + 'static {
  async fn join_member(
    &self,
    member: RegistryMember,
    core_runtime: CoreRuntime,
  ) -> Result<RegistryWatch, RegistryError>;

  async fn leave_member(&self, cluster_name: &str, node_address: &str) -> Result<(), RegistryError>;

  async fn register_client(&self, _cluster_name: &str, _client_id: &str) -> Result<(), RegistryError> {
    Ok(())
  }

  async fn heartbeat(&self, _cluster_name: &str, _node_address: &str) -> Result<(), RegistryError> {
    Ok(())
  }
}

#[derive(Debug, Clone)]
pub struct RegistryMember {
  pub cluster_name: String,
  pub node_address: String,
  pub kinds: Vec<String>,
}

impl RegistryMember {
  fn new(cluster_name: String, node_address: String, kinds: Vec<String>) -> Self {
    Self {
      cluster_name,
      node_address,
      kinds,
    }
  }
}

#[derive(Debug)]
pub struct RegistryWatch {
  pub initial_topology: ClusterTopology,
  updates: BroadcastStream<ClusterTopology>,
}

impl RegistryWatch {
  fn new(initial_topology: ClusterTopology, receiver: broadcast::Receiver<ClusterTopology>) -> Self {
    Self {
      initial_topology,
      updates: BroadcastStream::new(receiver),
    }
  }

  fn into_stream(self) -> BroadcastStream<ClusterTopology> {
    self.updates
  }
}

#[derive(Debug, Error)]
pub enum RegistryError {
  #[error("registry operation failed: {0}")]
  OperationFailed(String),
  #[error("registry watch closed")]
  WatchClosed,
}

impl From<RegistryError> for ClusterProviderError {
  fn from(value: RegistryError) -> Self {
    ClusterProviderError::ProviderError(value.to_string())
  }
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
  event_stream: Arc<EventStream>,
  core_runtime: CoreRuntime,
) -> ClusterProviderContext {
  let kind_names = kinds.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
  ClusterProviderContext::new(
    cluster_name.to_string(),
    node_address,
    kind_names,
    partition_manager,
    event_stream,
    core_runtime,
  )
}

use dashmap::DashMap;

#[derive(Debug, Clone)]
struct RegisteredMember {
  member: RegistryMember,
  event_stream: Arc<EventStream>,
}

#[derive(Debug, Clone)]
pub struct GrpcRegistrySettings {
  pub heartbeat_interval: Duration,
  pub heartbeat_timeout: Duration,
}

impl Default for GrpcRegistrySettings {
  fn default() -> Self {
    Self {
      heartbeat_interval: Duration::from_secs(5),
      heartbeat_timeout: Duration::from_secs(15),
    }
  }
}

pub struct GrpcRegistryClusterProvider<R: RegistryClient> {
  registry: Arc<R>,
  partition_managers: RwLock<HashMap<String, Weak<PartitionManager>>>,
  members: RwLock<HashMap<String, RegisteredMember>>,
  watch_tasks: RwLock<HashMap<String, Arc<dyn CoreJoinHandle>>>,
  heartbeat_tasks: RwLock<HashMap<String, Arc<dyn CoreJoinHandle>>>,
  settings: GrpcRegistrySettings,
}

impl<R: RegistryClient> fmt::Debug for GrpcRegistryClusterProvider<R> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("GrpcRegistryClusterProvider").finish()
  }
}

impl<R: RegistryClient> GrpcRegistryClusterProvider<R> {
  pub fn new(registry: Arc<R>) -> Self {
    Self::with_settings(registry, GrpcRegistrySettings::default())
  }

  pub fn with_settings(registry: Arc<R>, settings: GrpcRegistrySettings) -> Self {
    Self {
      registry,
      partition_managers: RwLock::new(HashMap::new()),
      members: RwLock::new(HashMap::new()),
      watch_tasks: RwLock::new(HashMap::new()),
      heartbeat_tasks: RwLock::new(HashMap::new()),
      settings,
    }
  }

  async fn register_watch(
    &self,
    cluster_name: String,
    node_address: String,
    partition_manager: Arc<PartitionManager>,
    event_stream: Arc<EventStream>,
    core_runtime: CoreRuntime,
    mut stream: BroadcastStream<ClusterTopology>,
  ) {
    let cluster_for_log = cluster_name.clone();
    let address_for_log = node_address.clone();
    let spawner = core_runtime.spawner();
    let future: CoreTaskFuture = Box::pin(async move {
      while let Some(result) = stream.next().await {
        match result {
          Ok(topology) => {
            let members = topology.members.clone();
            partition_manager.update_topology(topology).await;
            event_stream
              .publish(MessageHandle::new(TopologyEvent::snapshot(&cluster_for_log, members)))
              .await;
          }
          Err(err) => {
            warn!(?err, cluster = %cluster_for_log, node = %address_for_log, "registry watch terminated");
            break;
          }
        }
      }
    });

    match spawner.spawn(future) {
      Ok(handle) => {
        let mut tasks = self.watch_tasks.write().await;
        if let Some(existing) = tasks.insert(node_address, handle) {
          existing.cancel();
        }
      }
      Err(err) => {
        warn!(error = ?err, cluster = %cluster_name, node = %node_address, "failed to spawn registry watch");
      }
    }
  }

  async fn start_heartbeat(&self, cluster_name: String, node_address: String, core_runtime: CoreRuntime) {
    let interval = self.settings.heartbeat_interval;
    let registry = self.registry.clone();
    let cluster_clone = cluster_name.clone();
    let node_clone = node_address.clone();
    let spawner = core_runtime.spawner();
    let future: CoreTaskFuture = Box::pin(async move {
      let mut ticker = tokio::time::interval(interval);
      loop {
        ticker.tick().await;
        if let Err(err) = registry.heartbeat(&cluster_clone, &node_clone).await {
          warn!(?err, cluster = %cluster_clone, node = %node_clone, "failed to send heartbeat");
        }
      }
    });

    match spawner.spawn(future) {
      Ok(handle) => {
        if let Some(existing) = self.heartbeat_tasks.write().await.insert(node_address, handle) {
          existing.cancel();
        }
      }
      Err(err) => {
        warn!(error = ?err, cluster = %cluster_name, node = %node_address, "failed to spawn heartbeat task");
      }
    }
  }

  async fn stop_heartbeat(&self, node_address: &str) {
    if let Some(handle) = self.heartbeat_tasks.write().await.remove(node_address) {
      handle.cancel();
    }
  }

  #[cfg(test)]
  async fn stop_heartbeat_for_test(&self, node_address: &str) {
    self.stop_heartbeat(node_address).await;
  }

  async fn register_node(&self, ctx: &ClusterProviderContext) -> Result<RegistryWatch, ClusterProviderError> {
    let member = RegistryMember::new(ctx.cluster_name.clone(), ctx.node_address.clone(), ctx.kinds.clone());
    let watch = self.registry.join_member(member.clone(), ctx.core_runtime()).await?;

    {
      let mut managers = self.partition_managers.write().await;
      managers.insert(ctx.node_address.clone(), Arc::downgrade(&ctx.partition_manager));
    }

    {
      let mut members = self.members.write().await;
      members.insert(
        ctx.node_address.clone(),
        RegisteredMember {
          member,
          event_stream: ctx.event_stream(),
        },
      );
    }

    ctx
      .publish_topology_event(TopologyEvent::registered(
        &ctx.cluster_name,
        &ctx.node_address,
        &ctx.kinds,
      ))
      .await;

    Ok(watch)
  }

  async fn deregister_node(&self, cluster_name: &str, node_address: &str) -> Result<(), ClusterProviderError> {
    self.registry.leave_member(cluster_name, node_address).await?;
    let mut members = self.members.write().await;
    if let Some(registered) = members.remove(node_address) {
      registered
        .event_stream
        .publish(MessageHandle::new(TopologyEvent::deregistered(
          cluster_name,
          node_address,
        )))
        .await;
    }
    self.stop_heartbeat(node_address).await;
    Ok(())
  }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GrpcRegistryClient {
  uri: String,
}

#[allow(dead_code)]
impl GrpcRegistryClient {
  pub fn new(uri: impl Into<String>) -> Self {
    Self { uri: uri.into() }
  }

  fn endpoint(&self) -> Result<Endpoint, RegistryError> {
    Endpoint::from_shared(self.uri.clone())
      .map(|endpoint| {
        endpoint
          .tcp_keepalive(Some(Duration::from_secs(5)))
          .http2_keep_alive_interval(Duration::from_secs(5))
          .keep_alive_timeout(Duration::from_secs(2))
      })
      .map_err(|err| RegistryError::OperationFailed(err.to_string()))
  }

  async fn client(&self) -> Result<ClusterRegistryClient<Channel>, RegistryError> {
    let endpoint = self.endpoint()?;
    endpoint
      .connect()
      .await
      .map(ClusterRegistryClient::new)
      .map_err(|err| RegistryError::OperationFailed(err.to_string()))
  }

  fn topology_from_members(members: Vec<ProtoMember>) -> ClusterTopology {
    let converted = members
      .into_iter()
      .map(|member| ClusterMember::new(member.node_address, member.kinds))
      .collect();
    ClusterTopology { members: converted }
  }

  fn update_from_watch(watch: WatchUpdate) -> ClusterTopology {
    Self::topology_from_members(watch.members)
  }
}

impl From<tonic::transport::Error> for RegistryError {
  fn from(value: tonic::transport::Error) -> Self {
    RegistryError::OperationFailed(value.to_string())
  }
}

impl From<Status> for RegistryError {
  fn from(value: Status) -> Self {
    RegistryError::OperationFailed(value.to_string())
  }
}

#[async_trait]
impl RegistryClient for GrpcRegistryClient {
  async fn join_member(
    &self,
    member: RegistryMember,
    core_runtime: CoreRuntime,
  ) -> Result<RegistryWatch, RegistryError> {
    let mut client = self.client().await?;

    let request = JoinRequest {
      member: Some(ProtoMember {
        cluster_name: member.cluster_name.clone(),
        node_address: member.node_address.clone(),
        kinds: member.kinds.clone(),
      }),
    };

    let mut stream = client.join(request).await?.into_inner();

    let first_update = stream.message().await?.ok_or_else(|| RegistryError::WatchClosed)?;

    let topology = Self::update_from_watch(first_update);
    let (sender, receiver) = broadcast::channel(32);
    let _ = sender.send(topology.clone());

    let mut stream_for_task = stream;
    let sender_for_task = sender.clone();
    let future: CoreTaskFuture = Box::pin(async move {
      loop {
        match stream_for_task.message().await {
          Ok(Some(update)) => {
            let topology = GrpcRegistryClient::update_from_watch(update);
            let _ = sender_for_task.send(topology);
          }
          Ok(None) => break,
          Err(status) => {
            warn!(?status, "registry watch terminated");
            break;
          }
        }
      }
    });

    let spawner = core_runtime.spawner();
    match spawner.spawn(future) {
      Ok(handle) => handle.detach(),
      Err(err) => {
        warn!(error = ?err, cluster = %member.cluster_name, node = %member.node_address, "failed to spawn registry watch task");
        return Err(RegistryError::OperationFailed("failed to spawn registry watch".into()));
      }
    }

    Ok(RegistryWatch::new(topology, receiver))
  }

  async fn leave_member(&self, cluster_name: &str, node_address: &str) -> Result<(), RegistryError> {
    let mut client = self.client().await?;
    let request = LeaveRequest {
      cluster_name: cluster_name.to_string(),
      node_address: node_address.to_string(),
    };
    client.leave(request).await?;
    Ok(())
  }

  async fn register_client(&self, cluster_name: &str, client_id: &str) -> Result<(), RegistryError> {
    let mut client = self.client().await?;
    let request = RegisterClientRequest {
      cluster_name: cluster_name.to_string(),
      client_id: client_id.to_string(),
    };
    client.register_client(request).await?;
    Ok(())
  }

  async fn heartbeat(&self, cluster_name: &str, node_address: &str) -> Result<(), RegistryError> {
    let mut client = self.client().await?;
    let request = HeartbeatRequest {
      cluster_name: cluster_name.to_string(),
      node_address: node_address.to_string(),
    };
    client.heartbeat(request).await?;
    Ok(())
  }
}

#[async_trait]
impl<R: RegistryClient> ClusterProvider for GrpcRegistryClusterProvider<R> {
  async fn start_member(&self, ctx: &ClusterProviderContext) -> Result<(), ClusterProviderError> {
    let watch = self.register_node(ctx).await?;

    ctx
      .partition_manager
      .update_topology(watch.initial_topology.clone())
      .await;

    ctx
      .publish_topology_event(TopologyEvent::snapshot(
        &ctx.cluster_name,
        watch.initial_topology.members.clone(),
      ))
      .await;

    let stream = watch.into_stream();
    let pm = ctx.partition_manager.clone();
    let node_address = ctx.node_address.clone();
    self
      .register_watch(
        ctx.cluster_name.clone(),
        node_address,
        pm,
        ctx.event_stream(),
        ctx.core_runtime(),
        stream,
      )
      .await;

    self
      .start_heartbeat(ctx.cluster_name.clone(), ctx.node_address.clone(), ctx.core_runtime())
      .await;

    Ok(())
  }

  async fn start_client(&self, ctx: &ClusterProviderContext) -> Result<(), ClusterProviderError> {
    self
      .registry
      .register_client(&ctx.cluster_name, &ctx.node_address)
      .await?;
    Ok(())
  }

  async fn shutdown(&self, graceful: bool) -> Result<(), ClusterProviderError> {
    let addresses: Vec<String> = {
      let members = self.members.read().await;
      members.keys().cloned().collect()
    };

    for address in addresses {
      if graceful {
        if let Some(member) = {
          let members = self.members.read().await;
          members.get(&address).map(|entry| entry.member.clone())
        } {
          self.deregister_node(&member.cluster_name, &member.node_address).await?;
        }
      } else {
        let mut members = self.members.write().await;
        members.remove(&address);
        self.stop_heartbeat(&address).await;
      }

      if let Some(handle) = {
        let mut tasks = self.watch_tasks.write().await;
        tasks.remove(&address)
      } {
        handle.cancel();
      }

      let mut managers = self.partition_managers.write().await;
      managers.remove(&address);
    }

    Ok(())
  }

  async fn resolve_partition_manager(&self, address: &str) -> Option<Arc<PartitionManager>> {
    let guard = self.partition_managers.read().await;
    guard.get(address).and_then(|weak| weak.upgrade())
  }
}

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

#[cfg(test)]
mod tests {
  use super::broadcast;
  use super::registry_server::{spawn_registry_server, GrpcRegistryServerConfig};
  use super::*;
  use crate::cluster::Cluster;
  use crate::config::{ClusterConfig, RemoteOptions};
  use crate::kind::ClusterKind;
  use crate::virtual_actor::{VirtualActor, VirtualActorContext, VirtualActorRuntime};
  use nexus_actor_std_rs::actor::actor_system::ActorSystem;
  use nexus_actor_std_rs::actor::core::ActorError;
  use nexus_actor_std_rs::actor::message::MessageHandle;
  use std::collections::{HashMap, HashSet};
  use std::env;
  use std::net::SocketAddr;
  use std::sync::Arc;
  use tokio::sync::Mutex as TokioMutex;
  use tokio::time::{sleep, timeout, Duration};

  #[derive(Debug)]
  struct RegistryClusterState {
    members: HashMap<String, Vec<String>>,
    clients: HashSet<String>,
    sender: broadcast::Sender<ClusterTopology>,
  }

  #[derive(Debug)]
  struct MockRegistryClient {
    inner: TokioMutex<HashMap<String, RegistryClusterState>>,
  }

  impl MockRegistryClient {
    fn new() -> Self {
      Self {
        inner: TokioMutex::new(HashMap::new()),
      }
    }

    fn allocate_port() -> u16 {
      let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("allocate port");
      let port = listener.local_addr().expect("local addr").port();
      drop(listener);
      port
    }

    async fn list_members(&self, cluster_name: &str) -> Vec<String> {
      let guard = self.inner.lock().await;
      guard
        .get(cluster_name)
        .map(|state| state.members.keys().cloned().collect())
        .unwrap_or_default()
    }

    fn build_topology(members: &HashMap<String, Vec<String>>) -> ClusterTopology {
      let cluster_members = members
        .iter()
        .map(|(address, kinds)| ClusterMember::new(address.clone(), kinds.clone()))
        .collect();
      ClusterTopology {
        members: cluster_members,
      }
    }
  }

  #[async_trait]
  impl RegistryClient for MockRegistryClient {
    async fn join_member(
      &self,
      member: RegistryMember,
      _core_runtime: CoreRuntime,
    ) -> Result<RegistryWatch, RegistryError> {
      let mut guard = self.inner.lock().await;
      let state = guard.entry(member.cluster_name.clone()).or_insert_with(|| {
        let (sender, _) = broadcast::channel(32);
        RegistryClusterState {
          members: HashMap::new(),
          clients: HashSet::new(),
          sender,
        }
      });

      state.members.insert(member.node_address.clone(), member.kinds.clone());

      let receiver = state.sender.subscribe();
      let topology = Self::build_topology(&state.members);
      let _ = state.sender.send(topology.clone());

      Ok(RegistryWatch::new(topology, receiver))
    }

    async fn leave_member(&self, cluster_name: &str, node_address: &str) -> Result<(), RegistryError> {
      let mut guard = self.inner.lock().await;
      if let Some(state) = guard.get_mut(cluster_name) {
        state.members.remove(node_address);
        let topology = Self::build_topology(&state.members);
        let _ = state.sender.send(topology);
        if state.members.is_empty() && state.clients.is_empty() {
          guard.remove(cluster_name);
        }
      }
      Ok(())
    }

    async fn register_client(&self, cluster_name: &str, client_id: &str) -> Result<(), RegistryError> {
      let mut guard = self.inner.lock().await;
      let state = guard.entry(cluster_name.to_string()).or_insert_with(|| {
        let (sender, _) = broadcast::channel(32);
        RegistryClusterState {
          members: HashMap::new(),
          clients: HashSet::new(),
          sender,
        }
      });
      state.clients.insert(client_id.to_string());
      Ok(())
    }
  }

  #[derive(Debug)]
  struct DummyActor;

  #[async_trait]
  impl VirtualActor for DummyActor {
    async fn activate(&mut self, _ctx: &VirtualActorContext) -> Result<(), ActorError> {
      Ok(())
    }

    async fn handle(&mut self, _message: MessageHandle, _runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError> {
      Ok(())
    }
  }

  fn time_factor() -> f64 {
    env::var("TIME_FACTOR")
      .ok()
      .and_then(|value| value.parse::<f64>().ok())
      .filter(|factor| *factor > 0.0)
      .unwrap_or(1.0)
  }

  fn scaled_duration(base: Duration) -> Duration {
    let nanos = base.as_secs_f64() * time_factor();
    Duration::from_secs_f64(nanos)
  }

  async fn wait_for_event<F>(
    events: &Arc<TokioMutex<Vec<TopologyEvent>>>,
    timeout_duration: Duration,
    condition: F,
  ) -> bool
  where
    F: Fn(&[TopologyEvent]) -> bool, {
    timeout(timeout_duration, async {
      loop {
        let snapshot = events.lock().await.clone();
        if condition(&snapshot) {
          return true;
        }
        drop(snapshot);
        sleep(Duration::from_millis(25)).await;
      }
    })
    .await
    .unwrap_or(false)
  }

  #[tokio::test]
  async fn grpc_registry_provider_shares_topology() {
    let registry = Arc::new(MockRegistryClient::new());
    let provider = Arc::new(GrpcRegistryClusterProvider::new(registry.clone())) as Arc<dyn ClusterProvider>;

    let system_a = Arc::new(ActorSystem::new().await.expect("system a"));
    let system_b = Arc::new(ActorSystem::new().await.expect("system b"));

    let port_a = MockRegistryClient::allocate_port();
    let port_b = MockRegistryClient::allocate_port();

    let cluster_a = Cluster::new(
      system_a.clone(),
      ClusterConfig::new("grpc-registry")
        .with_provider(provider.clone())
        .with_remote_options(
          RemoteOptions::new("127.0.0.1", port_a).with_advertised_address(format!("127.0.0.1:{port_a}")),
        ),
    );
    let cluster_b = Cluster::new(
      system_b.clone(),
      ClusterConfig::new("grpc-registry")
        .with_provider(provider.clone())
        .with_remote_options(
          RemoteOptions::new("127.0.0.1", port_b).with_advertised_address(format!("127.0.0.1:{port_b}")),
        ),
    );

    cluster_a.register_kind(ClusterKind::virtual_actor("dummy", move |_identity| async {
      DummyActor
    }));
    cluster_b.register_kind(ClusterKind::virtual_actor("dummy", move |_identity| async {
      DummyActor
    }));

    cluster_a.start_member().await.expect("start member a");
    cluster_b.start_member().await.expect("start member b");

    sleep(Duration::from_millis(200)).await;

    let pm_a = cluster_a.partition_manager();
    let addr_a = system_a.get_address().await;
    let addr_b = system_b.get_address().await;

    let mut owners = HashSet::new();
    for idx in 0..32 {
      let identity = format!("id-{idx}");
      if let Some(owner) = pm_a.owner_for("dummy", &identity) {
        owners.insert(owner);
      }
    }

    assert!(owners.contains(&addr_a));
    assert!(owners.contains(&addr_b));

    assert_eq!(registry.list_members("grpc-registry").await.len(), 2);

    cluster_a.shutdown(true).await.expect("shutdown a");
    cluster_a.shutdown(false).await.expect("shutdown a");
    cluster_b.shutdown(true).await.expect("shutdown b");

    assert!(registry.list_members("grpc-registry").await.is_empty());
  }

  #[tokio::test]
  async fn grpc_registry_provider_emits_topology_events() {
    let registry = Arc::new(MockRegistryClient::new());
    let provider = Arc::new(GrpcRegistryClusterProvider::new(registry.clone())) as Arc<dyn ClusterProvider>;

    let system_a = Arc::new(ActorSystem::new().await.expect("system a"));
    let system_b = Arc::new(ActorSystem::new().await.expect("system b"));

    let events = Arc::new(TokioMutex::new(Vec::<TopologyEvent>::new()));
    let event_stream = system_a.get_event_stream().await;
    let events_clone = events.clone();
    let subscription = event_stream
      .subscribe(move |message| {
        let events_clone = events_clone.clone();
        async move {
          if let Some(event) = message.to_typed::<TopologyEvent>() {
            events_clone.lock().await.push(event);
          }
        }
      })
      .await;

    let port_a = MockRegistryClient::allocate_port();
    let port_b = MockRegistryClient::allocate_port();

    let cluster_a = Cluster::new(
      system_a.clone(),
      ClusterConfig::new("grpc-registry-events")
        .with_provider(provider.clone())
        .with_remote_options(
          RemoteOptions::new("127.0.0.1", port_a).with_advertised_address(format!("127.0.0.1:{port_a}")),
        ),
    );
    let cluster_b = Cluster::new(
      system_b.clone(),
      ClusterConfig::new("grpc-registry-events")
        .with_provider(provider.clone())
        .with_remote_options(
          RemoteOptions::new("127.0.0.1", port_b).with_advertised_address(format!("127.0.0.1:{port_b}")),
        ),
    );

    cluster_a.register_kind(ClusterKind::virtual_actor("dummy", move |_identity| async {
      DummyActor
    }));
    cluster_b.register_kind(ClusterKind::virtual_actor("dummy", move |_identity| async {
      DummyActor
    }));

    cluster_a.start_member().await.expect("start member a");
    cluster_b.start_member().await.expect("start member b");

    let remote_addr = system_b.get_address().await;

    assert!(
      wait_for_event(&events, scaled_duration(Duration::from_secs(3)), |items| {
        items
          .iter()
          .any(|event| event.event_type == "registered" && event.node_address.is_some())
      })
      .await,
      "registered event should be emitted",
    );

    assert!(
      wait_for_event(&events, scaled_duration(Duration::from_secs(3)), |items| {
        items
          .iter()
          .filter(|event| event.event_type == "snapshot")
          .any(|event| event.members.iter().any(|member| member.address == remote_addr))
      })
      .await,
      "snapshot event should include remote member",
    );

    cluster_a.shutdown(true).await.expect("shutdown a");
    cluster_b.shutdown(true).await.expect("shutdown b");

    assert!(
      wait_for_event(&events, scaled_duration(Duration::from_secs(3)), |items| {
        items
          .iter()
          .any(|event| event.event_type == "deregistered" && event.node_address.is_some())
      })
      .await,
      "deregistered event should be emitted when shutting down",
    );

    system_a.get_event_stream().await.unsubscribe(subscription).await;
  }

  #[tokio::test]
  async fn grpc_registry_provider_real_server_handles_heartbeat_timeout() {
    let addr = SocketAddr::new("127.0.0.1".parse().unwrap(), MockRegistryClient::allocate_port());
    let server_config = GrpcRegistryServerConfig {
      heartbeat_timeout: scaled_duration(Duration::from_millis(600)),
      cleanup_interval: scaled_duration(Duration::from_millis(200)),
      http2_keepalive_interval: scaled_duration(Duration::from_millis(200)),
      http2_keepalive_timeout: scaled_duration(Duration::from_millis(100)),
    };
    let (_service, server_handle) = spawn_registry_server(addr, server_config.clone());

    let client = Arc::new(GrpcRegistryClient::new(format!("http://{}", addr)));
    let grpc_provider = Arc::new(GrpcRegistryClusterProvider::with_settings(
      client.clone(),
      GrpcRegistrySettings {
        heartbeat_interval: scaled_duration(Duration::from_millis(150)),
        heartbeat_timeout: server_config.heartbeat_timeout,
      },
    ));
    let provider = grpc_provider.clone() as Arc<dyn ClusterProvider>;

    let system_a = Arc::new(ActorSystem::new().await.expect("system a"));
    let system_b = Arc::new(ActorSystem::new().await.expect("system b"));

    let cluster_a = Cluster::new(
      system_a.clone(),
      ClusterConfig::new("grpc-registry-e2e")
        .with_provider(provider.clone())
        .with_remote_options(RemoteOptions::new("127.0.0.1", MockRegistryClient::allocate_port())),
    );
    let cluster_b = Cluster::new(
      system_b.clone(),
      ClusterConfig::new("grpc-registry-e2e")
        .with_provider(provider.clone())
        .with_remote_options(RemoteOptions::new("127.0.0.1", MockRegistryClient::allocate_port())),
    );

    cluster_a.register_kind(ClusterKind::virtual_actor("dummy", move |_identity| async {
      DummyActor
    }));
    cluster_b.register_kind(ClusterKind::virtual_actor("dummy", move |_identity| async {
      DummyActor
    }));

    cluster_a.start_member().await.expect("start member a");
    cluster_b.start_member().await.expect("start member b");

    let remote_addr = system_b.get_address().await;

    // Wait until the remote node is observed.
    let events = Arc::new(TokioMutex::new(Vec::<TopologyEvent>::new()));
    let event_stream = system_a.get_event_stream().await;
    let events_clone = events.clone();
    let subscription = event_stream
      .subscribe(move |message| {
        let events_clone = events_clone.clone();
        async move {
          if let Some(event) = message.to_typed::<TopologyEvent>() {
            events_clone.lock().await.push(event);
          }
        }
      })
      .await;

    assert!(
      wait_for_event(&events, scaled_duration(Duration::from_secs(3)), |items| {
        items
          .iter()
          .filter(|event| event.event_type == "snapshot")
          .any(|event| event.members.iter().any(|member| member.address == remote_addr))
      })
      .await,
      "snapshot should include remote node",
    );

    // Simulate node failure by stopping heartbeat without deregistration.
    let addr_a = system_a.get_address().await;
    grpc_provider.stop_heartbeat_for_test(&addr_a).await;

    assert!(
      wait_for_event(&events, scaled_duration(Duration::from_secs(5)), |items| {
        items
          .iter()
          .filter(|event| event.event_type == "snapshot")
          .any(|event| event.members.iter().all(|member| member.address != addr_a))
      })
      .await,
      "snapshot should drop failed node",
    );

    cluster_b.shutdown(true).await.expect("shutdown b");
    system_a.get_event_stream().await.unsubscribe(subscription).await;
    server_handle.abort();
  }
}

#[allow(dead_code)]
#[cfg(test)]
pub mod registry_server {
  use super::*;
  use nexus_actor_core_rs::runtime::{CoreJoinHandle, CoreSpawner, CoreTaskFuture};
  use nexus_utils_std_rs::runtime::TokioCoreSpawner;

  use std::net::SocketAddr;
  use std::time::{Duration, Instant};

  use tokio::sync::{broadcast, RwLock};
  use tokio_stream::wrappers::UnboundedReceiverStream;
  use tonic::transport::Server;
  use tonic::{async_trait, Response, Status};

  use crate::generated::registry::cluster_registry_server::{ClusterRegistry, ClusterRegistryServer};
  use crate::generated::registry::{
    HeartbeatRequest, JoinRequest, LeaveRequest, Member, RegisterClientRequest, WatchUpdate,
  };

  #[allow(dead_code)]
  #[derive(Debug, Clone)]
  pub struct GrpcRegistryServerConfig {
    pub heartbeat_timeout: Duration,
    pub cleanup_interval: Duration,
    pub http2_keepalive_interval: Duration,
    pub http2_keepalive_timeout: Duration,
  }

  impl Default for GrpcRegistryServerConfig {
    fn default() -> Self {
      Self {
        heartbeat_timeout: Duration::from_secs(15),
        cleanup_interval: Duration::from_secs(5),
        http2_keepalive_interval: Duration::from_secs(5),
        http2_keepalive_timeout: Duration::from_secs(2),
      }
    }
  }

  #[allow(dead_code)]
  #[derive(Debug)]
  struct NodeEntry {
    member: Member,
    last_seen: Instant,
  }

  #[allow(dead_code)]
  #[derive(Debug)]
  struct ClusterState {
    members: HashMap<String, NodeEntry>,
    clients: HashSet<String>,
    broadcaster: broadcast::Sender<WatchUpdate>,
  }

  impl ClusterState {
    fn new() -> Self {
      let (sender, _) = broadcast::channel(32);
      Self {
        members: HashMap::new(),
        clients: HashSet::new(),
        broadcaster: sender,
      }
    }

    fn snapshot(&self) -> WatchUpdate {
      let members = self
        .members
        .values()
        .map(|entry| entry.member.clone())
        .collect::<Vec<_>>();
      WatchUpdate { members }
    }
  }

  #[allow(dead_code)]
  #[derive(Debug)]
  struct RegistryState {
    clusters: RwLock<HashMap<String, ClusterState>>,
    heartbeat_timeout: Duration,
    cleanup_interval: Duration,
  }

  #[allow(dead_code)]
  #[derive(Clone, Debug)]
  pub struct GrpcRegistryService {
    state: Arc<RegistryState>,
  }

  impl GrpcRegistryService {
    pub fn new(config: &GrpcRegistryServerConfig) -> Self {
      let state = Arc::new(RegistryState {
        clusters: RwLock::new(HashMap::new()),
        heartbeat_timeout: config.heartbeat_timeout,
        cleanup_interval: config.cleanup_interval,
      });
      let service = Self { state };
      service.spawn_cleanup_task();
      service
    }

    fn spawn_cleanup_task(&self) {
      let state = self.state.clone();
      let spawner = TokioCoreSpawner::current();
      let future: CoreTaskFuture = Box::pin(async move {
        let mut ticker = tokio::time::interval(state.cleanup_interval);
        loop {
          ticker.tick().await;
          let now = Instant::now();
          let mut clusters = state.clusters.write().await;
          for (_, cluster_state) in clusters.iter_mut() {
            let mut expired = Vec::new();
            for (address, entry) in cluster_state.members.iter() {
              if now.duration_since(entry.last_seen) > state.heartbeat_timeout {
                expired.push(address.clone());
              }
            }
            for address in expired {
              cluster_state.members.remove(&address);
            }
            let update = cluster_state.snapshot();
            let _ = cluster_state.broadcaster.send(update);
          }
        }
      });
      if let Ok(handle) = spawner.spawn(future) {
        handle.detach();
      }
    }

    async fn upsert_member(&self, member: Member) -> WatchUpdate {
      let mut clusters = self.state.clusters.write().await;
      let cluster_state = clusters
        .entry(member.cluster_name.clone())
        .or_insert_with(ClusterState::new);
      cluster_state.members.insert(
        member.node_address.clone(),
        NodeEntry {
          member: member.clone(),
          last_seen: Instant::now(),
        },
      );
      let update = cluster_state.snapshot();
      let _ = cluster_state.broadcaster.send(update.clone());
      update
    }

    async fn update_heartbeat(&self, cluster_name: &str, node_address: &str) -> Result<(), Status> {
      let mut clusters = self.state.clusters.write().await;
      let Some(cluster_state) = clusters.get_mut(cluster_name) else {
        return Err(Status::not_found("cluster not registered"));
      };
      let Some(entry) = cluster_state.members.get_mut(node_address) else {
        return Err(Status::not_found("node not registered"));
      };
      entry.last_seen = Instant::now();
      Ok(())
    }

    async fn remove_member(&self, cluster_name: &str, node_address: &str) {
      let mut clusters = self.state.clusters.write().await;
      if let Some(cluster_state) = clusters.get_mut(cluster_name) {
        cluster_state.members.remove(node_address);
        let update = cluster_state.snapshot();
        let _ = cluster_state.broadcaster.send(update);
        if cluster_state.members.is_empty() && cluster_state.clients.is_empty() {
          clusters.remove(cluster_name);
        }
      }
    }

    async fn add_client(&self, cluster_name: &str, client_id: &str) {
      let mut clusters = self.state.clusters.write().await;
      let cluster_state = clusters
        .entry(cluster_name.to_string())
        .or_insert_with(ClusterState::new);
      cluster_state.clients.insert(client_id.to_string());
    }

    async fn subscribe(&self, cluster_name: &str) -> broadcast::Receiver<WatchUpdate> {
      let clusters = self.state.clusters.read().await;
      let cluster_state = clusters
        .get(cluster_name)
        .expect("cluster state must exist when subscribing");
      cluster_state.broadcaster.subscribe()
    }

    pub fn into_service(self) -> ClusterRegistryServer<Self> {
      ClusterRegistryServer::new(self)
    }

    pub async fn serve(
      self,
      addr: SocketAddr,
      config: &GrpcRegistryServerConfig,
    ) -> Result<(), tonic::transport::Error> {
      Server::builder()
        .tcp_keepalive(Some(config.http2_keepalive_interval))
        .http2_keepalive_interval(Some(config.http2_keepalive_interval))
        .http2_keepalive_timeout(Some(config.http2_keepalive_timeout))
        .add_service(self.clone().into_service())
        .serve(addr)
        .await
    }
  }

  #[async_trait]
  impl ClusterRegistry for GrpcRegistryService {
    type JoinStream = UnboundedReceiverStream<Result<WatchUpdate, Status>>;

    async fn join(&self, request: tonic::Request<JoinRequest>) -> Result<Response<Self::JoinStream>, Status> {
      let member = request
        .into_inner()
        .member
        .ok_or_else(|| Status::invalid_argument("member is required"))?;

      let snapshot = self.upsert_member(member.clone()).await;

      let mut receiver = self.subscribe(&member.cluster_name).await;
      let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
      if tx.send(Ok(snapshot.clone())).is_err() {
        return Err(Status::internal("failed to deliver snapshot"));
      }

      let service = self.clone();
      let spawner = TokioCoreSpawner::current();
      let future: CoreTaskFuture = Box::pin(async move {
        loop {
          match receiver.recv().await {
            Ok(update) => {
              if tx.send(Ok(update.clone())).is_err() {
                service.remove_member(&member.cluster_name, &member.node_address).await;
                break;
              }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => {
              let _ = tx.send(Ok(WatchUpdate { members: Vec::new() }));
              break;
            }
          }
        }
      });
      if let Ok(handle) = spawner.spawn(future) {
        handle.detach();
      }

      Ok(Response::new(UnboundedReceiverStream::new(rx)))
    }

    async fn leave(&self, request: tonic::Request<LeaveRequest>) -> Result<Response<()>, Status> {
      let req = request.into_inner();
      self.remove_member(&req.cluster_name, &req.node_address).await;
      Ok(Response::new(()))
    }

    async fn register_client(&self, request: tonic::Request<RegisterClientRequest>) -> Result<Response<()>, Status> {
      let req = request.into_inner();
      self.add_client(&req.cluster_name, &req.client_id).await;
      Ok(Response::new(()))
    }

    async fn heartbeat(&self, request: tonic::Request<HeartbeatRequest>) -> Result<Response<()>, Status> {
      let req = request.into_inner();
      self.update_heartbeat(&req.cluster_name, &req.node_address).await?;
      Ok(Response::new(()))
    }
  }

  #[allow(dead_code)]
  pub struct RegistryServerHandle {
    handle: Arc<dyn CoreJoinHandle>,
  }

  impl RegistryServerHandle {
    pub fn abort(self) {
      self.handle.cancel();
    }
  }

  pub fn spawn_registry_server(
    addr: SocketAddr,
    config: GrpcRegistryServerConfig,
  ) -> (GrpcRegistryService, RegistryServerHandle) {
    let service = GrpcRegistryService::new(&config);
    let cloned_service = service.clone();
    let spawner = TokioCoreSpawner::current();
    let future: CoreTaskFuture = Box::pin(async move {
      let _ = cloned_service.serve(addr, &config).await;
    });
    let handle = spawner.spawn(future).expect("failed to spawn registry server task");
    (service, RegistryServerHandle { handle })
  }
}

#[cfg(test)]
#[allow(unused_imports)]
pub use registry_server::{spawn_registry_server, GrpcRegistryServerConfig, GrpcRegistryService, RegistryServerHandle};
