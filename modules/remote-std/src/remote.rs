#[cfg(test)]
mod tests;

use crate::block_list::BlockList;
use crate::config::server_config::ServerConfig;
use crate::config::Config;
use crate::endpoint_manager::{EndpointManager, EndpointStatisticsSnapshot};
use crate::endpoint_reader::EndpointReader;
use crate::endpoint_state::ConnectionState;
use crate::generated::remote::remoting_server::RemotingServer;
use crate::generated::remote::{ActorPidRequest, ActorPidResponse};
use crate::messages::RemoteDeliver;
use crate::remote_process::RemoteProcess;
use crate::response_status_code::{ActorPidResponseExt, ResponseError, ResponseStatusCode};
use crate::serializer::initialize_proto_serializers;
use crate::serializer::SerializerId;
use async_trait::async_trait;
use dashmap::DashMap;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::SenderPart;
use nexus_actor_std_rs::actor::core::ExtendedPid;
use nexus_actor_std_rs::actor::core::Props;
use nexus_actor_std_rs::actor::message::{MessageHandle, ReadonlyMessageHeadersHandle};
use nexus_actor_std_rs::actor::metrics::metrics_impl::MetricsSink;
use nexus_actor_std_rs::actor::process::future::ActorFutureError;
use nexus_actor_std_rs::actor::process::process_registry::AddressResolver;
use nexus_actor_std_rs::actor::process::ProcessHandle;
use nexus_actor_std_rs::extensions::{next_extension_id, Extension, ExtensionId};
use nexus_actor_std_rs::generated::actor::Pid;
use once_cell::sync::Lazy;
use std::any::Any;
use std::fmt;
use std::future::Future;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use tonic::transport::Server;

#[derive(Debug, Clone, Error)]
pub enum RemoteError {
  #[error("Server error")]
  ServerError,
}

#[derive(Debug, Clone, Error)]
pub enum RemoteSpawnError {
  #[error("activator request failed: {0}")]
  Request(#[from] ActorFutureError),
  #[error("activator returned unexpected payload")]
  InvalidResponse,
  #[error("activator response missing pid (status = {0:?})")]
  MissingPid(ResponseStatusCode),
  #[error("spawn rejected: {0}")]
  Status(#[from] ResponseError),
}

#[derive(Debug, Clone, Error)]
#[error("activation handler error: {status_code:?} - {reason}")]
pub struct ActivationHandlerError {
  status_code: ResponseStatusCode,
  reason: String,
}

impl ActivationHandlerError {
  pub fn new(status_code: ResponseStatusCode, reason: impl Into<String>) -> Self {
    Self {
      status_code,
      reason: reason.into(),
    }
  }

  pub fn status_code(&self) -> ResponseStatusCode {
    self.status_code
  }
}

#[async_trait]
pub trait ActivationHandler: Send + Sync + fmt::Debug + 'static {
  async fn activate(&self, kind: &str, identity: &str) -> Result<Option<Pid>, ActivationHandlerError>;
}

pub static EXTENSION_ID: Lazy<ExtensionId> = Lazy::new(next_extension_id);

#[derive(Debug, Clone)]
struct Shutdown {
  tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl Shutdown {
  fn new() -> (Self, oneshot::Receiver<()>) {
    let (tx, rx) = oneshot::channel();
    (
      Shutdown {
        tx: Arc::new(Mutex::new(Some(tx))),
      },
      rx,
    )
  }

  async fn shutdown(&self) {
    if let Some(tx) = self.tx.lock().await.take() {
      let _ = tx.send(());
    }
  }
}

#[derive(Debug)]
struct RemoteInner {
  actor_system: ActorSystem,
  endpoint_reader: Mutex<Option<EndpointReader>>,
  endpoint_manager: Mutex<Option<EndpointManager>>,
  config: Config,
  kinds: DashMap<String, Props>,
  block_list: BlockList,
  shutdown: Mutex<Option<Shutdown>>,
  metrics_sink: OnceCell<Option<Arc<MetricsSink>>>,
  activation_handler: Mutex<Option<Arc<dyn ActivationHandler>>>,
}

impl RemoteInner {
  async fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    self
      .metrics_sink
      .get_or_init(|| async {
        self
          .actor_system
          .metrics_runtime()
          .map(|runtime| Arc::new(runtime.sink_for_actor(Some("remote"))))
      })
      .await
      .clone()
  }

  async fn set_activation_handler(&self, handler: Arc<dyn ActivationHandler>) {
    let mut guard = self.activation_handler.lock().await;
    *guard = Some(handler);
  }

  async fn clear_activation_handler(&self) {
    let mut guard = self.activation_handler.lock().await;
    *guard = None;
  }

  async fn activation_handler(&self) -> Option<Arc<dyn ActivationHandler>> {
    let guard = self.activation_handler.lock().await;
    guard.clone()
  }
}

#[derive(Debug, Clone)]
pub struct Remote {
  inner: Arc<RemoteInner>,
}

impl Remote {
  pub async fn new(actor_system: ActorSystem, config: Config) -> Self {
    let remote = Remote {
      inner: Arc::new(RemoteInner {
        actor_system: actor_system.clone(),
        endpoint_reader: Mutex::new(None),
        endpoint_manager: Mutex::new(None),
        config: config.clone(),
        kinds: DashMap::new(),
        block_list: BlockList::new(),
        shutdown: Mutex::new(None),
        metrics_sink: OnceCell::new(),
        activation_handler: Mutex::new(None),
      }),
    };

    let kinds = config.get_kinds().await;
    for entry in kinds.iter() {
      remote.register(entry.key(), entry.value().clone());
    }

    actor_system
      .get_extensions()
      .await
      .register(Arc::new(Mutex::new(remote.clone())))
      .await;
    remote
  }

  async fn get_endpoint_reader(&self) -> EndpointReader {
    let mg = self.inner.endpoint_reader.lock().await;
    mg.as_ref().expect("EndpointReader is not found").clone()
  }

  async fn set_endpoint_reader(&self, endpoint_reader: EndpointReader) {
    let mut mg = self.inner.endpoint_reader.lock().await;
    *mg = Some(endpoint_reader);
  }

  pub(crate) async fn get_endpoint_manager_opt(&self) -> Option<EndpointManager> {
    let mg = self.inner.endpoint_manager.lock().await;
    mg.clone()
  }

  pub(crate) async fn get_endpoint_manager(&self) -> EndpointManager {
    let mg = self.inner.endpoint_manager.lock().await;
    mg.as_ref().expect("EndpointManager is not found").clone()
  }

  async fn set_endpoint_manager(&self, endpoint_manager: EndpointManager) {
    let mut mg = self.inner.endpoint_manager.lock().await;
    *mg = Some(endpoint_manager);
  }

  #[cfg(test)]
  pub(crate) async fn set_endpoint_manager_for_test(&self, endpoint_manager: EndpointManager) {
    self.set_endpoint_manager(endpoint_manager).await;
  }

  pub fn get_config(&self) -> &Config {
    &self.inner.config
  }

  pub fn get_kinds(&self) -> &DashMap<String, Props> {
    &self.inner.kinds
  }

  pub fn get_actor_system(&self) -> &ActorSystem {
    &self.inner.actor_system
  }

  pub fn get_block_list(&self) -> &BlockList {
    &self.inner.block_list
  }

  pub async fn metrics_sink(&self) -> Option<Arc<MetricsSink>> {
    self.inner.metrics_sink().await
  }

  pub async fn set_activation_handler(&self, handler: Arc<dyn ActivationHandler>) {
    self.inner.set_activation_handler(handler).await;
  }

  pub async fn clear_activation_handler(&self) {
    self.inner.clear_activation_handler().await;
  }

  pub async fn activation_handler(&self) -> Option<Arc<dyn ActivationHandler>> {
    self.inner.activation_handler().await
  }

  pub async fn get_endpoint_statistics(&self, address: &str) -> Option<EndpointStatisticsSnapshot> {
    self
      .get_endpoint_manager_opt()
      .await
      .and_then(|manager| manager.statistics_snapshot(address))
  }

  pub async fn await_reconnect(&self, address: &str) -> Option<ConnectionState> {
    let manager = self.get_endpoint_manager_opt().await?;
    Some(manager.await_reconnect(address).await)
  }

  pub fn register(&self, kind: &str, props: Props) {
    self.inner.kinds.insert(kind.to_string(), props);
  }

  pub fn get_known_kinds(&self) -> Vec<String> {
    self.inner.kinds.iter().map(|kv| kv.key().clone()).collect()
  }

  pub async fn spawn_remote_named(
    &self,
    address: &str,
    name: &str,
    kind: &str,
    timeout: Duration,
  ) -> Result<Pid, RemoteSpawnError> {
    let _ = initialize_proto_serializers::<ActorPidRequest>();
    let _ = initialize_proto_serializers::<ActorPidResponse>();

    let actor_system = self.get_actor_system().clone();
    let root = actor_system.get_root_context().await;
    let future = root
      .request_future(
        ExtendedPid::new(Pid::new(address, "activator")),
        MessageHandle::new(ActorPidRequest {
          kind: kind.to_string(),
          name: name.to_string(),
        }),
        timeout,
      )
      .await;

    let response_handle = future.result().await?;
    let actor_pid_response = response_handle
      .to_typed::<ActorPidResponse>()
      .ok_or(RemoteSpawnError::InvalidResponse)?;

    let status = actor_pid_response.status_code_enum();
    status.ensure_ok().map_err(RemoteSpawnError::from)?;
    let pid = actor_pid_response.pid.ok_or(RemoteSpawnError::MissingPid(status))?;

    Ok(pid)
  }

  pub async fn spawn_remote(&self, address: &str, kind: &str, timeout: Duration) -> Result<Pid, RemoteSpawnError> {
    self.spawn_remote_named(address, "", kind, timeout).await
  }

  pub async fn start(&self) -> Result<(), RemoteError> {
    self.start_with_callback(|| async {}).await
  }

  pub async fn start_with_callback<F, Fut>(&self, on_start: F) -> Result<(), RemoteError>
  where
    F: FnOnce() -> Fut,
    Fut: Future<Output = ()> + Send + Sync, {
    let (shutdown, rx) = Shutdown::new();
    {
      let mut mg = self.inner.shutdown.lock().await;
      *mg = Some(shutdown.clone());
    }

    let my_self = Arc::new(self.clone());
    let cloned_self = my_self.clone();
    let mut server = Server::builder();
    if let Some(sc) = &self.inner.config.get_server_config().await {
      server = Self::configure_server(server, sc);
    }

    let advertised_address = self.inner.config.get_advertised_address().await;
    let listen_host = self.inner.config.get_host().await.expect("Host is not found");
    let port = self.inner.config.get_port().await.expect("Port is not found");
    let socket_addrs = (listen_host.as_str(), port)
      .to_socket_addrs()
      .expect("Invalid host")
      .collect::<Vec<_>>();
    let socket_addr = socket_addrs
      .into_iter()
      .find(|addr| addr.is_ipv4())
      .expect("Failed to resolve hostname");

    let published_address = advertised_address.unwrap_or_else(|| format!("{}:{}", listen_host, port));
    tracing::debug!(
      "Binding to {:?}, published address = {}",
      socket_addr,
      published_address
    );

    let process_registry = self.inner.actor_system.get_process_registry().await;
    process_registry.register_address_resolver(AddressResolver::new(move |core_pid| {
      let cloned_self = cloned_self.clone();
      let (address, id, request_id) = core_pid.clone().into_parts();
      let pid = Pid {
        address,
        id,
        request_id,
      };
      async move {
        let (ph, _) = cloned_self.remote_handler(&pid).await;
        Some(ph)
      }
    }));
    process_registry.set_address(published_address);
    tracing::info!("Starting server: {}", socket_addr);

    let self_weak = Arc::downgrade(&my_self);

    let mut endpoint_manager = EndpointManager::new(self_weak.clone());
    endpoint_manager.start().await.map_err(|e| {
      tracing::error!("Failed to start EndpointManager: {:?}", e);
      RemoteError::ServerError
    })?;
    self.set_endpoint_manager(endpoint_manager.clone()).await;

    let endpoint_reader = EndpointReader::new(self_weak);
    self.set_endpoint_reader(endpoint_reader.clone()).await;

    let router = server.add_service(RemotingServer::new(endpoint_reader));
    let shutdown_future = async {
      tracing::info!("Server started: {}", socket_addr);
      on_start().await;
      rx.await.ok();
    };
    if router.serve_with_shutdown(socket_addr, shutdown_future).await.is_err() {
      return Err(RemoteError::ServerError);
    }
    Ok(())
  }

  fn configure_server(mut server: Server, sc: &ServerConfig) -> Server {
    if let Some(concurrency_limit_per_connection) = sc.concurrency_limit_per_connection {
      server = server.concurrency_limit_per_connection(concurrency_limit_per_connection);
    }
    if let Some(timeout) = sc.timeout {
      server = server.timeout(timeout);
    }
    server = server.initial_stream_window_size(sc.initial_stream_window_size);
    server = server.initial_connection_window_size(sc.initial_connection_window_size);
    server = server.max_concurrent_streams(sc.max_concurrent_streams);
    server = server.http2_keepalive_interval(sc.http2_keepalive_interval);
    server = server.http2_keepalive_timeout(sc.http2_keepalive_timeout);
    server = server.http2_adaptive_window(sc.http2_adaptive_window);
    server = server.http2_max_pending_accept_reset_streams(sc.http2_max_pending_accept_reset_streams);
    server = server.tcp_keepalive(sc.tcp_keepalive);
    server = server.tcp_nodelay(sc.tcp_nodelay);
    server = server.http2_max_header_list_size(sc.http2_max_header_list_size);
    server = server.max_frame_size(sc.max_frame_size);
    server = server.accept_http1(sc.accept_http1);
    server
  }

  async fn remote_handler(&self, pid: &Pid) -> (ProcessHandle, bool) {
    let ref_process = RemoteProcess::new(pid.clone(), self.clone());
    (ProcessHandle::new(ref_process.clone()), true)
  }

  pub async fn shutdown(&self, graceful: bool) -> Result<(), RemoteError> {
    if graceful {
      tracing::debug!("Shutting down gracefully");
      if let Some(mut endpoint_manager) = self.get_endpoint_manager_opt().await {
        endpoint_manager.stop().await.map_err(|e| {
          tracing::error!("Failed to stop EndpointManager: {:?}", e);
          RemoteError::ServerError
        })?;
      }
      self.get_endpoint_reader().await.set_suspend(true);
    }
    if let Some(shutdown) = self.inner.shutdown.lock().await.take() {
      shutdown.shutdown().await;
    }
    Ok(())
  }

  pub async fn send_message(
    &self,
    target: Pid,
    header: Option<ReadonlyMessageHeadersHandle>,
    message: MessageHandle,
    sender: Option<Pid>,
    serializer_id: SerializerId,
  ) {
    let rd = RemoteDeliver {
      header,
      message,
      target,
      sender,
      serializer_id: serializer_id.into(),
    };
    self.get_endpoint_manager().await.remote_deliver(rd).await
  }
}

impl Extension for Remote {
  fn extension_id(&self) -> ExtensionId {
    *EXTENSION_ID
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}
