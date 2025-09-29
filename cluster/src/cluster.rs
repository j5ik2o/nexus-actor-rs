use std::sync::Arc;

use dashmap::DashMap;
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{SenderPart, SpawnerPart, StopperPart};
use nexus_actor_core_rs::actor::core::ExtendedPid;
use nexus_actor_core_rs::actor::message::{Message, MessageHandle};
use nexus_actor_core_rs::actor::process::future::ActorFutureError;
use std::time::Duration;

use crate::config::ClusterConfig;
use crate::identity::ClusterIdentity;
use crate::kind::ClusterKind;
use crate::messages::ClusterInit;
use crate::provider::{provider_context_from_kinds, ClusterProvider, ClusterProviderContext, ClusterProviderError};
use crate::virtual_actor::VirtualActorContext;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ClusterError {
  #[error("kind not registered: {0}")]
  KindNotRegistered(String),
  #[error("request timed out after {0:?}")]
  RequestTimeout(Duration),
  #[error("request failed: {0}")]
  RequestFailed(ActorFutureError),
}

impl ClusterError {
  fn from_actor_future_error(error: ActorFutureError, timeout: Duration) -> Self {
    match error {
      ActorFutureError::TimeoutError => ClusterError::RequestTimeout(timeout),
      other => ClusterError::RequestFailed(other),
    }
  }
}

/// Virtual Actor を管理する最小限の Cluster 実装。
#[derive(Debug, Clone)]
pub struct Cluster {
  actor_system: Arc<ActorSystem>,
  config: ClusterConfig,
  provider: Arc<dyn ClusterProvider>,
  kinds: Arc<DashMap<String, ClusterKind>>,
  pid_cache: Arc<DashMap<ClusterIdentity, ExtendedPid>>,
}

impl Cluster {
  pub fn new(actor_system: Arc<ActorSystem>, config: ClusterConfig) -> Self {
    let provider = config.provider();
    Self {
      actor_system,
      config,
      provider,
      kinds: Arc::new(DashMap::new()),
      pid_cache: Arc::new(DashMap::new()),
    }
  }

  pub fn config(&self) -> &ClusterConfig {
    &self.config
  }

  pub fn provider(&self) -> Arc<dyn ClusterProvider> {
    self.provider.clone()
  }

  fn request_timeout(&self) -> Duration {
    self.config.request_timeout()
  }

  pub fn register_kind(&self, kind: ClusterKind) {
    self.kinds.insert(kind.name().to_string(), kind);
  }

  pub fn deregister_kind(&self, kind: &str) {
    self.kinds.remove(kind);
  }

  pub fn actor_system(&self) -> Arc<ActorSystem> {
    self.actor_system.clone()
  }

  fn provider_context(&self) -> ClusterProviderContext {
    provider_context_from_kinds(&self.config.cluster_name, &self.kinds)
  }

  pub async fn start_member(&self) -> Result<(), ClusterProviderError> {
    let ctx = self.provider_context();
    self.provider.start_member(&ctx).await
  }

  pub async fn start_client(&self) -> Result<(), ClusterProviderError> {
    let ctx = self.provider_context();
    self.provider.start_client(&ctx).await
  }

  pub async fn shutdown(&self, graceful: bool) -> Result<(), ClusterProviderError> {
    let mut root = self.actor_system.get_root_context().await;
    let pids = self
      .pid_cache
      .iter()
      .map(|entry| entry.value().clone())
      .collect::<Vec<_>>();

    for pid in &pids {
      let future = root.stop_future_with_timeout(pid, self.request_timeout()).await;
      let _ = future.result().await;
    }

    self.pid_cache.clear();
    self.provider.shutdown(graceful).await
  }

  pub async fn get(&self, identity: ClusterIdentity) -> Result<ExtendedPid, ClusterError> {
    if let Some(existing) = self.pid_cache.get(&identity) {
      return Ok(existing.clone());
    }

    let kind = self
      .kinds
      .get(identity.kind())
      .ok_or_else(|| ClusterError::KindNotRegistered(identity.kind().to_string()))?;

    let props = kind.props(&identity).await;
    let mut root = self.actor_system.get_root_context().await;
    let pid = root.spawn(props).await;

    let init = ClusterInit::new(VirtualActorContext::new(
      identity.clone(),
      self.actor_system(),
      self.clone(),
    ));
    root.send(pid.clone(), MessageHandle::new(init)).await;

    self.pid_cache.insert(identity, pid.clone());
    Ok(pid)
  }

  pub fn forget(&self, identity: &ClusterIdentity) {
    self.pid_cache.remove(identity);
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
  use std::sync::Arc;
  use std::time::Duration;
  use tokio::sync::{oneshot, Mutex};

  use crate::provider::InMemoryClusterProvider;
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
    assert_eq!(members, vec!["test-cluster".to_string()]);

    cluster.start_client().await.expect("start client");
    let clients = provider.clients_snapshot().await;
    assert_eq!(clients, vec!["test-cluster".to_string()]);

    cluster.shutdown(true).await.expect("shutdown");
  }
}
