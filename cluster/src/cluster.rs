use std::sync::Arc;

use async_trait::async_trait;
use dashmap::DashMap;
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{SenderPart, StopperPart};
use nexus_actor_core_rs::actor::core::ExtendedPid;
use nexus_actor_core_rs::actor::message::{Message, MessageHandle};
use nexus_actor_core_rs::actor::process::future::ActorFutureError;
use nexus_actor_core_rs::generated::actor::Pid;
use nexus_actor_remote_rs::{
  ActivationHandler, ActivationHandlerError, Config as RemoteConfig, ConfigOption as RemoteConfigOption, Remote,
  ResponseStatusCode,
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
use crate::provider::{provider_context_from_kinds, ClusterProvider, ClusterProviderContext, ClusterProviderError};
use crate::rendezvous::ClusterMember;
use tokio::runtime::Handle;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;
use tracing::{error, warn};

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
#[derive(Debug, Clone)]
pub struct Cluster {
  actor_system: Arc<ActorSystem>,
  config: ClusterConfig,
  provider: Arc<dyn ClusterProvider>,
  identity_lookup: IdentityLookupHandle,
  kinds: Arc<DashMap<String, ClusterKind>>,
  partition_manager: Arc<PartitionManager>,
  remote: Arc<Mutex<Option<Arc<Remote>>>>,
  remote_task: Arc<Mutex<Option<JoinHandle<()>>>>,
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
    provider_context_from_kinds(
      &self.config.cluster_name,
      address,
      &self.kinds,
      self.partition_manager.clone(),
    )
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

    let handle = tokio::spawn(async move {
      let start_result = remote_clone
        .start_with_callback(|| async {
          let _ = started_tx.send(());
        })
        .await;
      if let Err(err) = start_result {
        error!(?err, "Remote server terminated with error");
      }
    });

    let _ = started_rx.await;

    {
      let mut guard = self.remote.lock().await;
      *guard = Some(remote_arc);
    }

    {
      let mut guard = self.remote_task.lock().await;
      *guard = Some(handle);
    }

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
    if Handle::try_current().is_err() {
      return;
    }
    let actor_system = self.actor_system.clone();
    let kinds = self.kinds.clone();
    let partition_manager = self.partition_manager.clone();
    tokio::spawn(async move {
      let address = actor_system.get_address().await;
      let kind_names = kinds.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
      let member = ClusterMember::new(address, kind_names);
      partition_manager
        .update_topology(ClusterTopology { members: vec![member] })
        .await;
    });
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
      let _ = handle.await;
    }
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
  ) -> Result<nexus_actor_core_rs::actor::process::actor_future::ActorFuture, ClusterError>
  where
    M: Message + Send + Sync + 'static, {
    let pid = self.get(identity).await?;
    let root = self.actor_system.get_root_context().await;
    Ok(root.request_future(pid, MessageHandle::new(message), timeout).await)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use async_trait::async_trait;
  use nexus_actor_core_rs::actor::core::{ActorError, ErrorReason};
  use nexus_actor_core_rs::actor::message::MessageHandle;
  use nexus_actor_core_rs::actor::process::actor_future::ActorFuture;
  use nexus_actor_message_derive_rs::Message as MessageDerive;
  use nexus_actor_remote_rs::initialize_json_serializers;
  use serde::{Deserialize, Serialize};
  use std::collections::HashSet;
  use std::net::TcpListener;
  use std::sync::Arc;
  use std::time::Duration;
  use tokio::sync::{oneshot, Mutex};

  use crate::config::RemoteOptions;
  use crate::identity::ClusterIdentity;
  use crate::identity_lookup::DistributedIdentityLookup;
  use crate::partition::manager::ClusterTopology;
  use crate::provider::InMemoryClusterProvider;
  use crate::rendezvous::ClusterMember;
  use crate::virtual_actor::{VirtualActor, VirtualActorContext, VirtualActorRuntime};
  use tokio::time::{sleep, timeout};

  #[derive(Debug, Clone, PartialEq, MessageDerive)]
  struct Ping(pub String);

  #[derive(Debug, Clone, PartialEq, MessageDerive)]
  struct Ask(pub String);

  #[derive(Debug, Clone, PartialEq, MessageDerive)]
  struct Answer(pub String);

  #[derive(Debug)]
  struct AskActor;

  #[async_trait]
  impl VirtualActor for AskActor {
    async fn activate(&mut self, _ctx: &VirtualActorContext) -> Result<(), ActorError> {
      Ok(())
    }

    async fn handle(&mut self, message: MessageHandle, runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError> {
      if let Some(ask) = message.as_typed::<Ask>() {
        let reply = Answer(format!("reply:{text}", text = ask.0));
        runtime.respond(reply).await;
        return Ok(());
      }
      Err(ActorError::of_receive_error(ErrorReason::from("unexpected message")))
    }
  }

  #[derive(Debug, Clone, PartialEq, MessageDerive, Serialize, Deserialize)]
  struct RemoteEchoRequest {
    text: String,
  }

  impl RemoteEchoRequest {
    fn new<T: Into<String>>(text: T) -> Self {
      Self { text: text.into() }
    }
  }

  #[derive(Debug, Clone, PartialEq, MessageDerive, Serialize, Deserialize)]
  struct RemoteEchoResponse {
    text: String,
  }

  impl RemoteEchoResponse {
    fn new<T: Into<String>>(text: T) -> Self {
      Self { text: text.into() }
    }
  }

  #[derive(Debug)]
  struct RemoteEchoActor;

  #[async_trait]
  impl VirtualActor for RemoteEchoActor {
    async fn activate(&mut self, _ctx: &VirtualActorContext) -> Result<(), ActorError> {
      Ok(())
    }

    async fn handle(&mut self, message: MessageHandle, runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError> {
      if let Some(request) = message.to_typed::<RemoteEchoRequest>() {
        runtime
          .respond(RemoteEchoResponse::new(format!("echo:{}", request.text)))
          .await;
        return Ok(());
      }

      Err(ActorError::of_receive_error(ErrorReason::from("unsupported message")))
    }
  }

  fn allocate_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("allocate port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);
    port
  }

  #[derive(Debug)]
  struct TestVirtualActor {
    init_probe: Arc<Mutex<Option<oneshot::Sender<String>>>>,
    message_probe: Arc<Mutex<Option<oneshot::Sender<String>>>>,
  }

  impl TestVirtualActor {
    fn new(
      init_probe: Arc<Mutex<Option<oneshot::Sender<String>>>>,
      message_probe: Arc<Mutex<Option<oneshot::Sender<String>>>>,
    ) -> Self {
      Self {
        init_probe,
        message_probe,
      }
    }
  }

  #[async_trait]
  impl VirtualActor for TestVirtualActor {
    async fn activate(&mut self, ctx: &VirtualActorContext) -> Result<(), ActorError> {
      let mut guard = self.init_probe.lock().await;
      if let Some(sender) = guard.take() {
        let _ = sender.send(ctx.identity().id().to_string());
      }
      Ok(())
    }

    async fn handle(&mut self, message: MessageHandle, _runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError> {
      if let Some(ping) = message.as_typed::<Ping>() {
        let mut guard = self.message_probe.lock().await;
        if let Some(sender) = guard.take() {
          let _ = sender.send(ping.0.clone());
        }
      }
      Ok(())
    }
  }

  #[tokio::test]
  async fn spawn_virtual_actor_once() {
    let system = Arc::new(ActorSystem::new().await.expect("actor system"));
    let cluster = Cluster::new(system.clone(), ClusterConfig::new("test"));
    let (tx, rx) = oneshot::channel();
    let init_probe = Arc::new(Mutex::new(Some(tx)));
    let message_probe = Arc::new(Mutex::new(None));

    cluster.register_kind(ClusterKind::virtual_actor("echo", move |identity| {
      let _ = identity; // identity is available for factory consumers
      let init_probe = init_probe.clone();
      let message_probe = message_probe.clone();
      async move { TestVirtualActor::new(init_probe, message_probe) }
    }));

    let identity = ClusterIdentity::new("echo", "a");
    let pid1 = cluster.get(identity.clone()).await.expect("pid1");
    let pid2 = cluster.get(identity.clone()).await.expect("pid2");

    assert_eq!(pid1, pid2);
    let received = rx.await.expect("cluster init");
    assert_eq!(received, "a");
  }

  #[tokio::test]
  async fn request_delivers_message() {
    let system = Arc::new(ActorSystem::new().await.expect("actor system"));
    let cluster = Cluster::new(system.clone(), ClusterConfig::new("test"));
    let init_probe = Arc::new(Mutex::new(None));
    let (tx, rx) = oneshot::channel();
    let message_probe = Arc::new(Mutex::new(Some(tx)));

    cluster.register_kind(ClusterKind::virtual_actor("req", move |identity| {
      let _ = identity;
      let init_probe = init_probe.clone();
      let message_probe = message_probe.clone();
      async move { TestVirtualActor::new(init_probe, message_probe) }
    }));

    let identity = ClusterIdentity::new("req", "actor");
    cluster
      .request(identity.clone(), Ping("hello".into()))
      .await
      .expect("request");

    let received = rx.await.expect("ping");
    assert_eq!(received, "hello");
  }

  #[tokio::test]
  async fn request_message_returns_response() {
    let system = Arc::new(ActorSystem::new().await.expect("actor system"));
    let cluster = Cluster::new(system.clone(), ClusterConfig::new("test"));
    cluster.register_kind(ClusterKind::virtual_actor(
      "ask",
      move |_identity| async move { AskActor },
    ));

    let identity = ClusterIdentity::new("ask", "two");
    let response = cluster
      .request_message(identity, Ask("hello".into()))
      .await
      .expect("response");
    let answer = response.to_typed::<Answer>().expect("Answer");
    assert_eq!(answer.0, "reply:hello");
  }

  #[tokio::test]
  async fn start_member_updates_partition_topology() {
    let system = Arc::new(ActorSystem::new().await.expect("actor system"));
    let cluster = Cluster::new(system.clone(), ClusterConfig::new("test"));
    cluster.register_kind(ClusterKind::virtual_actor(
      "echo",
      move |_identity| async move { AskActor },
    ));

    cluster.start_member().await.expect("start member");

    let owner = cluster.partition_manager().owner_for("echo", "alice").expect("owner");
    let address = system.get_address().await;
    assert_eq!(owner, address);
  }

  #[tokio::test]
  async fn request_future_waits_for_response() {
    let system = Arc::new(ActorSystem::new().await.expect("actor system"));
    let cluster = Cluster::new(system.clone(), ClusterConfig::new("test"));
    cluster.register_kind(ClusterKind::virtual_actor(
      "ask",
      move |_identity| async move { AskActor },
    ));

    let identity = ClusterIdentity::new("ask", "one");
    let fut: ActorFuture = cluster
      .request_future(identity, Ask("hi".into()), std::time::Duration::from_secs(1))
      .await
      .expect("future");

    let response = fut.result().await.expect("actor future");
    let answer = response.to_typed::<Answer>().expect("Answer");
    assert_eq!(answer.0, "reply:hi");
  }

  #[derive(Debug)]
  struct SilentActor;

  #[async_trait]
  impl VirtualActor for SilentActor {
    async fn activate(&mut self, _ctx: &VirtualActorContext) -> Result<(), ActorError> {
      Ok(())
    }

    async fn handle(&mut self, _message: MessageHandle, _runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError> {
      Ok(())
    }
  }

  #[tokio::test]
  async fn request_message_times_out() {
    let system = Arc::new(ActorSystem::new().await.expect("actor system"));
    let cluster = Cluster::new(
      system.clone(),
      ClusterConfig::new("test").with_request_timeout(Duration::from_secs(1)),
    );
    cluster.register_kind(ClusterKind::virtual_actor("silent", move |_identity| async move {
      SilentActor
    }));

    let identity = ClusterIdentity::new("silent", "one");
    let err = cluster
      .request_message_with_timeout(identity, Ask("ping".into()), Duration::from_millis(50))
      .await
      .expect_err("timeout");

    match err {
      ClusterError::RequestTimeout(d) => assert_eq!(d, Duration::from_millis(50)),
      other => panic!("expected timeout, got {other:?}"),
    }
  }

  #[tokio::test]
  async fn start_member_registers_in_provider() {
    let provider = Arc::new(InMemoryClusterProvider::new());
    let system = Arc::new(ActorSystem::new().await.expect("actor system"));
    let config = ClusterConfig::new("test-cluster").with_provider(provider.clone());
    let cluster = Cluster::new(system.clone(), config);

    cluster.start_member().await.expect("start member");
    let members = provider.members_snapshot().await;
    let address = system.get_address().await;
    assert_eq!(members, vec![address.clone()]);

    cluster.start_client().await.expect("start client");
    let clients = provider.clients_snapshot().await;
    assert_eq!(clients, vec!["test-cluster".to_string()]);

    cluster.shutdown(true).await.expect("shutdown");
  }

  #[tokio::test]
  async fn partition_topology_shared_across_clusters() {
    let provider = Arc::new(InMemoryClusterProvider::new());

    let system_a = ActorSystem::new().await.expect("system a");
    let system_b = ActorSystem::new().await.expect("system b");
    let system_a_arc = Arc::new(system_a.clone());
    let system_b_arc = Arc::new(system_b.clone());

    let cluster_a = Cluster::new(
      system_a_arc.clone(),
      ClusterConfig::new("cluster-a").with_provider(provider.clone()),
    );
    let cluster_b = Cluster::new(
      system_b_arc.clone(),
      ClusterConfig::new("cluster-b").with_provider(provider.clone()),
    );

    cluster_a.register_kind(ClusterKind::virtual_actor(
      "echo",
      move |_identity| async move { AskActor },
    ));
    cluster_b.register_kind(ClusterKind::virtual_actor(
      "echo",
      move |_identity| async move { AskActor },
    ));

    cluster_a.start_member().await.expect("start member a");
    cluster_b.start_member().await.expect("start member b");

    let addr_a = system_a.get_address().await;
    let addr_b = system_b.get_address().await;

    let topology = provider.topology_snapshot().await;
    let addresses: HashSet<_> = topology.iter().map(|member| member.address.clone()).collect();
    assert!(addresses.contains(&addr_a));
    assert!(addresses.contains(&addr_b));

    let pm_a = cluster_a.partition_manager();
    let pm_b = cluster_b.partition_manager();

    let mut owners_a = HashSet::new();
    let mut owners_b = HashSet::new();
    for idx in 0..64 {
      let identity = format!("id-{idx}");
      if let Some(owner) = pm_a.owner_for("echo", &identity) {
        owners_a.insert(owner);
      }
      if let Some(owner) = pm_b.owner_for("echo", &identity) {
        owners_b.insert(owner);
      }
    }

    assert!(owners_a.contains(&addr_a));
    assert!(owners_a.contains(&addr_b));
    assert!(owners_b.contains(&addr_a));
    assert!(owners_b.contains(&addr_b));

    cluster_a.shutdown(true).await.expect("shutdown a");
    cluster_b.shutdown(true).await.expect("shutdown b");
  }

  #[tokio::test]
  async fn test_rebalance_moves_activation_to_new_owner() {
    let provider = Arc::new(InMemoryClusterProvider::new());

    let system_a = ActorSystem::new().await.expect("system a");
    let system_b = ActorSystem::new().await.expect("system b");
    let system_a_arc = Arc::new(system_a.clone());
    let system_b_arc = Arc::new(system_b.clone());

    let cluster_a = Cluster::new(
      system_a_arc.clone(),
      ClusterConfig::new("cluster-rebalance").with_provider(provider.clone()),
    );
    let cluster_b = Cluster::new(
      system_b_arc.clone(),
      ClusterConfig::new("cluster-rebalance").with_provider(provider.clone()),
    );

    cluster_a.register_kind(ClusterKind::virtual_actor(
      "ask",
      move |_identity| async move { AskActor },
    ));
    cluster_b.register_kind(ClusterKind::virtual_actor(
      "ask",
      move |_identity| async move { AskActor },
    ));

    cluster_a.start_member().await.expect("start member a");
    cluster_b.start_member().await.expect("start member b");

    let addr_a = system_a.get_address().await;
    let addr_b = system_b.get_address().await;

    let pm_a = cluster_a.partition_manager();
    let pm_b = cluster_b.partition_manager();

    let identity_key = (0..2048)
      .map(|idx| format!("id-{idx}"))
      .find(|id| pm_a.owner_for("ask", id.as_str()) == Some(addr_b.clone()))
      .expect("identity for remote owner");

    let identity = ClusterIdentity::new("ask", identity_key.clone());
    let lookup_a = cluster_a.identity_lookup();

    let remote_pid = lookup_a
      .get(&identity)
      .await
      .expect("lookup remote")
      .expect("remote pid");
    assert_eq!(remote_pid.address(), addr_b);

    let distributed_a = lookup_a
      .as_any()
      .downcast_ref::<DistributedIdentityLookup>()
      .expect("distributed lookup a");
    let cached_remote = distributed_a
      .snapshot()
      .into_iter()
      .find(|(cached_identity, _)| cached_identity == &identity)
      .expect("cached identity entry");
    assert_eq!(cached_remote.1.address(), addr_b);

    let single_member = ClusterMember::new(addr_a.clone(), vec!["ask".to_string()]);
    pm_a
      .update_topology(ClusterTopology {
        members: vec![single_member.clone()],
      })
      .await;
    pm_b
      .update_topology(ClusterTopology {
        members: vec![single_member],
      })
      .await;

    let identity_for_poll = identity.clone();
    let addr_a_for_poll = addr_a.clone();
    let lookup_for_poll = lookup_a.clone();

    let local_pid = timeout(Duration::from_secs(3), async move {
      loop {
        if let Some(distributed) = lookup_for_poll.as_any().downcast_ref::<DistributedIdentityLookup>() {
          if let Some((_, pid)) = distributed
            .snapshot()
            .into_iter()
            .find(|(cached_identity, _)| cached_identity == &identity_for_poll)
          {
            if pid.address() == addr_a_for_poll {
              return pid;
            }
          }
        }
        sleep(Duration::from_millis(50)).await;
      }
    })
    .await
    .expect("rebalance to local owner within timeout");

    assert_eq!(local_pid.address(), addr_a);

    let lookup_b = cluster_b.identity_lookup();
    let distributed_b = lookup_b
      .as_any()
      .downcast_ref::<DistributedIdentityLookup>()
      .expect("distributed lookup b");
    let retained_remote = distributed_b
      .snapshot()
      .into_iter()
      .any(|(cached_identity, _)| cached_identity == identity);
    assert!(!retained_remote, "remote cache should be cleared after handoff");

    cluster_a.shutdown(true).await.expect("shutdown a");
    cluster_b.shutdown(true).await.expect("shutdown b");
  }

  #[tokio::test]
  async fn remote_activation_request_roundtrip() {
    initialize_json_serializers::<RemoteEchoRequest>().expect("register request serializer");
    initialize_json_serializers::<RemoteEchoResponse>().expect("register response serializer");

    let provider = Arc::new(InMemoryClusterProvider::new());

    let port_a = allocate_port();
    let port_b = allocate_port();

    let system_a = Arc::new(ActorSystem::new().await.expect("system a"));
    let system_b = Arc::new(ActorSystem::new().await.expect("system b"));

    let cluster_a = Cluster::new(
      system_a.clone(),
      ClusterConfig::new("cluster-remote-a")
        .with_provider(provider.clone())
        .with_remote_options(RemoteOptions::new("127.0.0.1", port_a)),
    );
    let cluster_b = Cluster::new(
      system_b.clone(),
      ClusterConfig::new("cluster-remote-b")
        .with_provider(provider.clone())
        .with_remote_options(RemoteOptions::new("127.0.0.1", port_b)),
    );

    cluster_a.register_kind(ClusterKind::virtual_actor("remote-echo", move |_identity| async {
      RemoteEchoActor
    }));
    cluster_b.register_kind(ClusterKind::virtual_actor("remote-echo", move |_identity| async {
      RemoteEchoActor
    }));

    cluster_a.start_member().await.expect("start member a");
    cluster_b.start_member().await.expect("start member b");

    let addr_a = system_a.get_address().await;
    let addr_b = system_b.get_address().await;

    let partition_manager = cluster_a.partition_manager();
    let identity_key = loop {
      if let Some(candidate) = (0..1024).map(|idx| format!("remote-{idx}")).find(|candidate| {
        partition_manager
          .owner_for("remote-echo", candidate)
          .map(|owner| owner == addr_b)
          .unwrap_or(false)
      }) {
        break candidate;
      }

      tokio::time::sleep(Duration::from_millis(50)).await;
    };

    let identity = ClusterIdentity::new("remote-echo", identity_key);

    let response = cluster_a
      .request_message(identity.clone(), RemoteEchoRequest::new("hi"))
      .await
      .expect("remote request response");

    let echo = response.to_typed::<RemoteEchoResponse>().expect("typed response");
    assert_eq!(echo.text, "echo:hi");

    let lookup_handle = cluster_a.identity_lookup();
    let distributed = lookup_handle
      .as_any()
      .downcast_ref::<DistributedIdentityLookup>()
      .expect("distributed lookup");
    let cached = distributed
      .snapshot()
      .into_iter()
      .find(|(cached_identity, _)| cached_identity == &identity);
    let (_, pid) = cached.expect("cached remote pid");
    assert_eq!(pid.address(), addr_b);

    cluster_a.shutdown(true).await.expect("shutdown a");
    cluster_b.shutdown(true).await.expect("shutdown b");

    // ensure local system address unchanged
    assert_eq!(system_a.get_address().await, addr_a);
    assert_eq!(system_b.get_address().await, addr_b);
  }

  #[tokio::test]
  async fn request_spawns_on_remote_owner() {
    let provider = Arc::new(InMemoryClusterProvider::new());

    let system_a = ActorSystem::new().await.expect("system a");
    let system_b = ActorSystem::new().await.expect("system b");
    let system_a_arc = Arc::new(system_a.clone());
    let system_b_arc = Arc::new(system_b.clone());

    let cluster_a = Cluster::new(
      system_a_arc.clone(),
      ClusterConfig::new("cluster-a")
        .with_provider(provider.clone())
        .with_remote_options(RemoteOptions::new("127.0.0.1", 20081).with_advertised_address("127.0.0.1:20081")),
    );
    let cluster_b = Cluster::new(
      system_b_arc.clone(),
      ClusterConfig::new("cluster-b")
        .with_provider(provider.clone())
        .with_remote_options(RemoteOptions::new("127.0.0.1", 20082).with_advertised_address("127.0.0.1:20082")),
    );

    cluster_a.register_kind(ClusterKind::virtual_actor(
      "echo",
      move |_identity| async move { AskActor },
    ));
    cluster_b.register_kind(ClusterKind::virtual_actor(
      "echo",
      move |_identity| async move { AskActor },
    ));

    cluster_a.start_member().await.expect("start member a");
    cluster_b.start_member().await.expect("start member b");

    let addr_b = system_b.get_address().await;
    let pm_a = cluster_a.partition_manager();

    let identity_key = (0..1000)
      .map(|idx| format!("id-{idx}"))
      .find(|id| pm_a.owner_for("echo", id.as_str()) == Some(addr_b.clone()))
      .expect("identity for remote owner");

    let identity = ClusterIdentity::new("echo", identity_key.clone());
    let pid = cluster_a
      .partition_manager()
      .activate(identity.clone())
      .await
      .expect("activate remote")
      .expect("remote pid");
    assert_eq!(pid.address(), addr_b);

    cluster_a.shutdown(true).await.expect("shutdown a");
    cluster_b.shutdown(true).await.expect("shutdown b");
  }
}
