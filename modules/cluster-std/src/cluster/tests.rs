#![allow(clippy::too_many_lines)]

use super::*;
use async_trait::async_trait;
use nexus_actor_std_rs::actor::core::{ActorError, ErrorReason};
use nexus_actor_std_rs::actor::message::MessageHandle;
use nexus_actor_std_rs::actor::process::actor_future::ActorFuture;
use nexus_actor_std_rs::actor::process::future::ActorFutureError;
use nexus_message_derive_rs::Message as MessageDerive;
use nexus_remote_std_rs::{initialize_json_serializers, initialize_proto_serializers};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};
use tokio::time::{sleep, timeout};

use crate::config::RemoteOptions;
use crate::identity::ClusterIdentity;
use crate::identity_lookup::DistributedIdentityLookup;
use crate::partition::manager::ClusterTopology;
use crate::provider::InMemoryClusterProvider;
use crate::rendezvous::ClusterMember;
use crate::virtual_actor::{VirtualActor, VirtualActorContext, VirtualActorRuntime};

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

#[derive(Debug)]
struct RemoteSilentActor;

#[async_trait]
impl VirtualActor for RemoteSilentActor {
  async fn activate(&mut self, _ctx: &VirtualActorContext) -> Result<(), ActorError> {
    Ok(())
  }

  async fn handle(&mut self, _message: MessageHandle, _runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Clone, PartialEq, MessageDerive, prost::Message)]
struct RemoteProtoRequest {
  #[prost(string, tag = "1")]
  text: String,
}

impl RemoteProtoRequest {
  fn new<T: Into<String>>(text: T) -> Self {
    Self { text: text.into() }
  }
}

#[derive(Clone, PartialEq, MessageDerive, prost::Message)]
struct RemoteProtoResponse {
  #[prost(string, tag = "1")]
  text: String,
}

impl RemoteProtoResponse {
  fn new<T: Into<String>>(text: T) -> Self {
    Self { text: text.into() }
  }
}

#[derive(Debug)]
struct RemoteProtoActor;

#[async_trait]
impl VirtualActor for RemoteProtoActor {
  async fn activate(&mut self, _ctx: &VirtualActorContext) -> Result<(), ActorError> {
    Ok(())
  }

  async fn handle(&mut self, message: MessageHandle, runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError> {
    if let Some(request) = message.to_typed::<RemoteProtoRequest>() {
      runtime
        .respond(RemoteProtoResponse::new(format!("proto:{}", request.text)))
        .await;
      return Ok(());
    }

    Err(ActorError::of_receive_error(ErrorReason::from("unsupported message")))
  }
}

#[derive(Debug, Clone, PartialEq, MessageDerive, Serialize, Deserialize)]
struct MissingSerializerRequest {
  text: String,
}

impl MissingSerializerRequest {
  fn new<T: Into<String>>(text: T) -> Self {
    Self { text: text.into() }
  }
}

#[derive(Debug)]
struct MissingSerializerActor;

#[async_trait]
impl VirtualActor for MissingSerializerActor {
  async fn activate(&mut self, _ctx: &VirtualActorContext) -> Result<(), ActorError> {
    Ok(())
  }

  async fn handle(&mut self, message: MessageHandle, _runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError> {
    if message.to_typed::<MissingSerializerRequest>().is_some() {
      // deliberately do nothing to emulate an actor that ignores the request
      return Ok(());
    }

    Err(ActorError::of_receive_error(ErrorReason::from("unsupported message")))
  }
}

#[derive(Debug)]
struct RemoteFailingActor;

#[async_trait]
impl VirtualActor for RemoteFailingActor {
  async fn activate(&mut self, _ctx: &VirtualActorContext) -> Result<(), ActorError> {
    Ok(())
  }

  async fn handle(&mut self, _message: MessageHandle, _runtime: VirtualActorRuntime<'_>) -> Result<(), ActorError> {
    Err(ActorError::of_receive_error(ErrorReason::from("intentional failure")))
  }
}

fn allocate_port() -> u16 {
  let listener = TcpListener::bind("127.0.0.1:0").expect("allocate port");
  let port = listener.local_addr().expect("local addr").port();
  drop(listener);
  port
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

  let identity = ClusterIdentity::new("ask", "one");
  let fut: ActorFuture = cluster
    .request_future(identity, Ask("hi".into()), Duration::from_secs(1))
    .await
    .expect("future");

  let response = fut.result().await.expect("actor future");
  let answer = response.to_typed::<Answer>().expect("Answer");
  assert_eq!(answer.0, "reply:hi");
}

#[tokio::test]
async fn request_message_times_out() {
  let system = Arc::new(ActorSystem::new().await.expect("actor system"));
  let cluster = Cluster::new(
    system.clone(),
    ClusterConfig::new("test").with_request_timeout(Duration::from_secs(1)),
  );
  cluster.register_kind(ClusterKind::virtual_actor("silent", move |_identity| async move {
    RemoteSilentActor
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
  let lookup_for_poll = cluster_a.identity_lookup();

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
async fn test_activate_local_populates_lookup_cache() {
  let provider = Arc::new(InMemoryClusterProvider::new());

  let system = ActorSystem::new().await.expect("actor system");
  let system_arc = Arc::new(system.clone());

  let cluster = Cluster::new(
    system_arc.clone(),
    ClusterConfig::new("cluster-local-cache").with_provider(provider.clone()),
  );

  cluster.register_kind(ClusterKind::virtual_actor(
    "ask",
    move |_identity| async move { AskActor },
  ));

  cluster.start_member().await.expect("start member");

  let address = system.get_address().await;
  let partition_manager = cluster.partition_manager();

  partition_manager
    .update_topology(ClusterTopology {
      members: vec![ClusterMember::new(address.clone(), vec!["ask".to_string()])],
    })
    .await;

  let identity = ClusterIdentity::new("ask", "cache-target");

  let pid = partition_manager
    .activate_local(identity.clone())
    .await
    .expect("activation succeeds")
    .expect("pid returned");
  assert_eq!(pid.address(), address);

  let lookup = cluster.identity_lookup();
  let distributed = lookup
    .as_any()
    .downcast_ref::<DistributedIdentityLookup>()
    .expect("distributed lookup");
  let cached = distributed
    .snapshot()
    .into_iter()
    .find(|(cached_identity, _)| cached_identity == &identity)
    .expect("cached identity");
  assert_eq!(cached.1.address(), address);

  cluster.shutdown(true).await.expect("shutdown");
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

    sleep(Duration::from_millis(50)).await;
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

  assert_eq!(system_a.get_address().await, addr_a);
  assert_eq!(system_b.get_address().await, addr_b);
}

#[tokio::test]
async fn remote_activation_request_repeated_roundtrip() {
  initialize_json_serializers::<RemoteEchoRequest>().expect("register request serializer");
  initialize_json_serializers::<RemoteEchoResponse>().expect("register response serializer");

  let provider = Arc::new(InMemoryClusterProvider::new());

  let port_a = allocate_port();
  let port_b = allocate_port();

  let system_a = Arc::new(ActorSystem::new().await.expect("system a"));
  let system_b = Arc::new(ActorSystem::new().await.expect("system b"));

  let cluster_a = Cluster::new(
    system_a.clone(),
    ClusterConfig::new("cluster-remote-repeat-a")
      .with_provider(provider.clone())
      .with_remote_options(RemoteOptions::new("127.0.0.1", port_a)),
  );
  let cluster_b = Cluster::new(
    system_b.clone(),
    ClusterConfig::new("cluster-remote-repeat-b")
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
    if let Some(candidate) = (0..2048).map(|idx| format!("repeat-{idx}")).find(|candidate| {
      partition_manager
        .owner_for("remote-echo", candidate)
        .map(|owner| owner == addr_b)
        .unwrap_or(false)
    }) {
      break candidate;
    }
    sleep(Duration::from_millis(50)).await;
  };

  let identity = ClusterIdentity::new("remote-echo", identity_key.clone());

  let mut last_pid = None;
  let lookup_a = cluster_a.identity_lookup();

  for round in 0..8 {
    let payload = format!("hi-{round}");
    let response = cluster_a
      .request_message(identity.clone(), RemoteEchoRequest::new(payload.clone()))
      .await
      .expect("remote response");

    let echo = response.to_typed::<RemoteEchoResponse>().expect("typed response");
    assert_eq!(echo.text, format!("echo:{payload}"));

    let distributed = lookup_a
      .as_any()
      .downcast_ref::<DistributedIdentityLookup>()
      .expect("distributed lookup a");
    let cached = distributed
      .snapshot()
      .into_iter()
      .find(|(cached_identity, _)| cached_identity == &identity)
      .expect("cached remote pid");
    if let Some(previous) = &last_pid {
      assert_eq!(cached.1, *previous, "remote pid should remain stable across requests");
    } else {
      last_pid = Some(cached.1.clone());
    }
    assert_eq!(cached.1.address(), addr_b);
  }

  let distributed_a = lookup_a
    .as_any()
    .downcast_ref::<DistributedIdentityLookup>()
    .expect("distributed lookup a final");
  let (_, cached_pid) = distributed_a
    .snapshot()
    .into_iter()
    .find(|(cached_identity, _)| cached_identity == &identity)
    .expect("cached remote pid final");
  assert_eq!(cached_pid.address(), addr_b);

  let lookup_b = cluster_b.identity_lookup();
  let distributed_b = lookup_b
    .as_any()
    .downcast_ref::<DistributedIdentityLookup>()
    .expect("distributed lookup b");
  let (_, local_pid) = distributed_b
    .snapshot()
    .into_iter()
    .find(|(cached_identity, _)| cached_identity == &identity)
    .expect("owner cache entry");
  assert_eq!(local_pid.address(), addr_b);

  cluster_a.shutdown(true).await.expect("shutdown a");
  cluster_b.shutdown(true).await.expect("shutdown b");

  assert_eq!(system_a.get_address().await, addr_a);
  assert_eq!(system_b.get_address().await, addr_b);
}

#[tokio::test]
async fn remote_activation_proto_roundtrip() {
  initialize_proto_serializers::<RemoteProtoRequest>().expect("register proto request");
  initialize_proto_serializers::<RemoteProtoResponse>().expect("register proto response");

  let provider = Arc::new(InMemoryClusterProvider::new());

  let port_a = allocate_port();
  let port_b = allocate_port();

  let system_a = Arc::new(ActorSystem::new().await.expect("system a"));
  let system_b = Arc::new(ActorSystem::new().await.expect("system b"));

  let cluster_a = Cluster::new(
    system_a.clone(),
    ClusterConfig::new("cluster-remote-proto-a")
      .with_provider(provider.clone())
      .with_remote_options(RemoteOptions::new("127.0.0.1", port_a)),
  );
  let cluster_b = Cluster::new(
    system_b.clone(),
    ClusterConfig::new("cluster-remote-proto-b")
      .with_provider(provider.clone())
      .with_remote_options(RemoteOptions::new("127.0.0.1", port_b)),
  );

  cluster_a.register_kind(ClusterKind::virtual_actor("remote-proto", move |_identity| async {
    RemoteProtoActor
  }));
  cluster_b.register_kind(ClusterKind::virtual_actor("remote-proto", move |_identity| async {
    RemoteProtoActor
  }));

  cluster_a.start_member().await.expect("start member a");
  cluster_b.start_member().await.expect("start member b");

  let addr_b = system_b.get_address().await;
  let partition_manager = cluster_a.partition_manager();
  let identity_key = loop {
    if let Some(candidate) = (0..1024).map(|idx| format!("proto-{idx}")).find(|candidate| {
      partition_manager
        .owner_for("remote-proto", candidate)
        .map(|owner| owner == addr_b)
        .unwrap_or(false)
    }) {
      break candidate;
    }

    sleep(Duration::from_millis(50)).await;
  };

  let identity = ClusterIdentity::new("remote-proto", identity_key);

  let response = cluster_a
    .request_message(identity.clone(), RemoteProtoRequest::new("hi"))
    .await
    .expect("proto response");

  let echo = response
    .to_typed::<RemoteProtoResponse>()
    .expect("proto typed response");
  assert_eq!(echo.text, "proto:hi");

  let lookup_handle = cluster_a.identity_lookup();
  let distributed = lookup_handle
    .as_any()
    .downcast_ref::<DistributedIdentityLookup>()
    .expect("distributed lookup");
  let cached = distributed
    .snapshot()
    .into_iter()
    .find(|(cached_identity, _)| cached_identity == &identity);
  let (_, pid) = cached.expect("cached proto pid");
  assert_eq!(pid.address(), addr_b);

  cluster_a.shutdown(true).await.expect("shutdown a");
  cluster_b.shutdown(true).await.expect("shutdown b");
}

#[tokio::test]
async fn remote_activation_timeout_propagates_error() {
  initialize_json_serializers::<RemoteEchoRequest>().expect("register request serializer");

  let provider = Arc::new(InMemoryClusterProvider::new());

  let port_a = allocate_port();
  let port_b = allocate_port();

  let system_a = Arc::new(ActorSystem::new().await.expect("system a"));
  let system_b = Arc::new(ActorSystem::new().await.expect("system b"));

  let cluster_a = Cluster::new(
    system_a.clone(),
    ClusterConfig::new("cluster-remote-timeout-a")
      .with_provider(provider.clone())
      .with_request_timeout(Duration::from_millis(200))
      .with_remote_options(RemoteOptions::new("127.0.0.1", port_a)),
  );
  let cluster_b = Cluster::new(
    system_b.clone(),
    ClusterConfig::new("cluster-remote-timeout-b")
      .with_provider(provider.clone())
      .with_request_timeout(Duration::from_millis(200))
      .with_remote_options(RemoteOptions::new("127.0.0.1", port_b)),
  );

  cluster_a.register_kind(ClusterKind::virtual_actor("remote-silent", move |_identity| async {
    RemoteSilentActor
  }));
  cluster_b.register_kind(ClusterKind::virtual_actor("remote-silent", move |_identity| async {
    RemoteSilentActor
  }));

  cluster_a.start_member().await.expect("start member a");
  cluster_b.start_member().await.expect("start member b");

  let addr_b = system_b.get_address().await;
  let partition_manager = cluster_a.partition_manager();
  let identity_key = loop {
    if let Some(candidate) = (0..1024).map(|idx| format!("silent-{idx}")).find(|candidate| {
      partition_manager
        .owner_for("remote-silent", candidate)
        .map(|owner| owner == addr_b)
        .unwrap_or(false)
    }) {
      break candidate;
    }

    sleep(Duration::from_millis(50)).await;
  };

  let identity = ClusterIdentity::new("remote-silent", identity_key);

  let err = cluster_a
    .request_message(identity, RemoteEchoRequest::new("timeout"))
    .await
    .expect_err("timeout error");

  match err {
    ClusterError::RequestTimeout(duration) => assert!(duration <= Duration::from_millis(200)),
    other => panic!("unexpected error: {other:?}"),
  }

  cluster_a.shutdown(true).await.expect("shutdown a");
  cluster_b.shutdown(true).await.expect("shutdown b");
}

#[tokio::test]
async fn remote_activation_failure_propagates_status() {
  initialize_json_serializers::<RemoteEchoRequest>().expect("register request serializer");

  let provider = Arc::new(InMemoryClusterProvider::new());

  let port_a = allocate_port();
  let port_b = allocate_port();

  let system_a = Arc::new(ActorSystem::new().await.expect("system a"));
  let system_b = Arc::new(ActorSystem::new().await.expect("system b"));

  let cluster_a = Cluster::new(
    system_a.clone(),
    ClusterConfig::new("cluster-remote-failure-a")
      .with_provider(provider.clone())
      .with_remote_options(RemoteOptions::new("127.0.0.1", port_a)),
  );
  let cluster_b = Cluster::new(
    system_b.clone(),
    ClusterConfig::new("cluster-remote-failure-b")
      .with_provider(provider.clone())
      .with_remote_options(RemoteOptions::new("127.0.0.1", port_b)),
  );

  cluster_a.register_kind(ClusterKind::virtual_actor("remote-fail", move |_identity| async {
    RemoteFailingActor
  }));
  cluster_b.register_kind(ClusterKind::virtual_actor("remote-fail", move |_identity| async {
    RemoteFailingActor
  }));

  cluster_a.start_member().await.expect("start member a");
  cluster_b.start_member().await.expect("start member b");

  let addr_b = system_b.get_address().await;
  let partition_manager = cluster_a.partition_manager();
  let identity_key = loop {
    if let Some(candidate) = (0..1024).map(|idx| format!("fail-{idx}")).find(|candidate| {
      partition_manager
        .owner_for("remote-fail", candidate)
        .map(|owner| owner == addr_b)
        .unwrap_or(false)
    }) {
      break candidate;
    }

    sleep(Duration::from_millis(50)).await;
  };

  let identity = ClusterIdentity::new("remote-fail", identity_key);

  let err = cluster_a
    .request_message(identity, RemoteEchoRequest::new("boom"))
    .await
    .expect_err("remote failure");

  match err {
    ClusterError::Partition(PartitionManagerError::RequestFailed(inner))
      if inner == ActorFutureError::DeadLetterError => {}
    ClusterError::RequestFailed(inner) if inner == ActorFutureError::DeadLetterError => {}
    ClusterError::RequestTimeout(_) => {}
    other => panic!("unexpected error variant: {other:?}"),
  }

  cluster_a.shutdown(true).await.expect("shutdown a");
  cluster_b.shutdown(true).await.expect("shutdown b");
}

#[tokio::test]
async fn remote_activation_missing_serializer_fails() {
  let provider = Arc::new(InMemoryClusterProvider::new());

  let port_a = allocate_port();
  let port_b = allocate_port();

  let system_a = Arc::new(ActorSystem::new().await.expect("system a"));
  let system_b = Arc::new(ActorSystem::new().await.expect("system b"));

  let cluster_a = Cluster::new(
    system_a.clone(),
    ClusterConfig::new("cluster-remote-missing-serializer-a")
      .with_provider(provider.clone())
      .with_request_timeout(Duration::from_millis(200))
      .with_remote_options(RemoteOptions::new("127.0.0.1", port_a)),
  );
  let cluster_b = Cluster::new(
    system_b.clone(),
    ClusterConfig::new("cluster-remote-missing-serializer-b")
      .with_provider(provider.clone())
      .with_request_timeout(Duration::from_millis(200))
      .with_remote_options(RemoteOptions::new("127.0.0.1", port_b)),
  );

  cluster_a.register_kind(ClusterKind::virtual_actor("remote-missing", move |_identity| async {
    MissingSerializerActor
  }));
  cluster_b.register_kind(ClusterKind::virtual_actor("remote-missing", move |_identity| async {
    MissingSerializerActor
  }));

  cluster_a.start_member().await.expect("start member a");
  cluster_b.start_member().await.expect("start member b");

  let addr_b = system_b.get_address().await;
  let partition_manager = cluster_a.partition_manager();
  let identity_key = loop {
    if let Some(candidate) = (0..1024).map(|idx| format!("missing-{idx}")).find(|candidate| {
      partition_manager
        .owner_for("remote-missing", candidate)
        .map(|owner| owner == addr_b)
        .unwrap_or(false)
    }) {
      break candidate;
    }

    sleep(Duration::from_millis(50)).await;
  };

  let identity = ClusterIdentity::new("remote-missing", identity_key);

  let err = cluster_a
    .request_message(identity, MissingSerializerRequest::new("hi"))
    .await
    .expect_err("missing serializer error");

  match err {
    ClusterError::Partition(PartitionManagerError::RequestFailed(_))
    | ClusterError::RequestFailed(_)
    | ClusterError::RequestTimeout(_) => {
      // acceptable: serialization failure should propagate as request failure or timeout
    }
    other => panic!("unexpected error variant: {other:?}"),
  }

  cluster_a.shutdown(true).await.expect("shutdown a");
  cluster_b.shutdown(true).await.expect("shutdown b");
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
