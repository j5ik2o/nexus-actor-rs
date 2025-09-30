use async_trait::async_trait;
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Weak};
use thiserror::Error;
use tokio::sync::{broadcast, RwLock};
use tokio::task::JoinHandle;
use tokio_stream::wrappers::BroadcastStream;

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

#[async_trait]
pub trait RegistryClient: Send + Sync + fmt::Debug + 'static {
  async fn join_member(&self, member: RegistryMember) -> Result<RegistryWatch, RegistryError>;

  async fn leave_member(&self, cluster_name: &str, node_address: &str) -> Result<(), RegistryError>;

  async fn register_client(&self, _cluster_name: &str, _client_id: &str) -> Result<(), RegistryError> {
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

#[allow(dead_code)]
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
) -> ClusterProviderContext {
  let kind_names = kinds.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
  ClusterProviderContext::new(cluster_name.to_string(), node_address, kind_names, partition_manager)
}

use dashmap::DashMap;

pub struct GrpcRegistryClusterProvider<R: RegistryClient> {
  registry: Arc<R>,
  partition_managers: RwLock<HashMap<String, Weak<PartitionManager>>>,
  members: RwLock<HashMap<String, RegistryMember>>,
  watch_tasks: RwLock<HashMap<String, JoinHandle<()>>>,
}

impl<R: RegistryClient> fmt::Debug for GrpcRegistryClusterProvider<R> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("GrpcRegistryClusterProvider").finish()
  }
}

impl<R: RegistryClient> GrpcRegistryClusterProvider<R> {
  pub fn new(registry: Arc<R>) -> Self {
    Self {
      registry,
      partition_managers: RwLock::new(HashMap::new()),
      members: RwLock::new(HashMap::new()),
      watch_tasks: RwLock::new(HashMap::new()),
    }
  }

  async fn register_watch(
    &self,
    node_address: String,
    partition_manager: Arc<PartitionManager>,
    mut stream: BroadcastStream<ClusterTopology>,
  ) {
    let handle = tokio::spawn(async move {
      while let Some(result) = stream.next().await {
        match result {
          Ok(topology) => partition_manager.update_topology(topology).await,
          Err(_) => break,
        }
      }
    });

    let mut tasks = self.watch_tasks.write().await;
    if let Some(existing) = tasks.insert(node_address, handle) {
      existing.abort();
    }
  }
}

#[async_trait]
impl<R: RegistryClient> ClusterProvider for GrpcRegistryClusterProvider<R> {
  async fn start_member(&self, ctx: &ClusterProviderContext) -> Result<(), ClusterProviderError> {
    let member = RegistryMember::new(ctx.cluster_name.clone(), ctx.node_address.clone(), ctx.kinds.clone());

    let watch = self.registry.join_member(member.clone()).await?;

    {
      let mut managers = self.partition_managers.write().await;
      managers.insert(ctx.node_address.clone(), Arc::downgrade(&ctx.partition_manager));
    }

    {
      let mut members = self.members.write().await;
      members.insert(ctx.node_address.clone(), member);
    }

    ctx
      .partition_manager
      .update_topology(watch.initial_topology.clone())
      .await;

    let stream = watch.into_stream();
    let pm = ctx.partition_manager.clone();
    let node_address = ctx.node_address.clone();
    self.register_watch(node_address, pm, stream).await;

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
    let mut tasks = self.watch_tasks.write().await;
    let mut managers = self.partition_managers.write().await;
    let mut members = self.members.write().await;

    let addresses: Vec<String> = members.keys().cloned().collect();

    for address in addresses {
      if graceful {
        if let Some(member) = members.get(&address) {
          self
            .registry
            .leave_member(&member.cluster_name, &member.node_address)
            .await?;
        }
      }

      if let Some(handle) = tasks.remove(&address) {
        handle.abort();
      }
      managers.remove(&address);
      members.remove(&address);
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
  use super::*;
  use crate::cluster::Cluster;
  use crate::config::{ClusterConfig, RemoteOptions};
  use crate::kind::ClusterKind;
  use crate::virtual_actor::{VirtualActor, VirtualActorContext, VirtualActorRuntime};
  use nexus_actor_core_rs::actor::actor_system::ActorSystem;
  use nexus_actor_core_rs::actor::core::ActorError;
  use nexus_actor_core_rs::actor::message::MessageHandle;
  use std::collections::{HashMap, HashSet};
  use std::sync::Arc;
  use tokio::sync::Mutex as TokioMutex;
  use tokio::time::{sleep, Duration};

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
    async fn join_member(&self, member: RegistryMember) -> Result<RegistryWatch, RegistryError> {
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
    cluster_b.shutdown(true).await.expect("shutdown b");

    assert!(registry.list_members("grpc-registry").await.is_empty());
  }
}
