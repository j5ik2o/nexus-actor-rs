#[cfg(test)]
mod tests;

use crate::block_list::BlockList;
use crate::config::server_config::ServerConfig;
use crate::config::Config;
use crate::endpoint_manager::{EndpointManager, EndpointStatisticsSnapshot};
use crate::endpoint_reader::EndpointReader;
use crate::endpoint_state::ConnectionState;
use crate::generated::remote::remoting_server::RemotingServer;
use crate::messages::RemoteDeliver;
use crate::remote_process::RemoteProcess;
use crate::serializer::SerializerId;
use dashmap::DashMap;
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::core::Props;
use nexus_actor_core_rs::actor::message::{MessageHandle, ReadonlyMessageHeadersHandle};
use nexus_actor_core_rs::actor::process::process_registry::AddressResolver;
use nexus_actor_core_rs::actor::process::ProcessHandle;
use nexus_actor_core_rs::extensions::{next_extension_id, Extension, ExtensionId};
use nexus_actor_core_rs::generated::actor::Pid;
use once_cell::sync::Lazy;
use std::any::Any;
use std::future::Future;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::oneshot;
use tokio::sync::Mutex;
use tonic::transport::Server;

#[derive(Debug, Clone, Error)]
pub enum RemoteError {
  #[error("Server error")]
  ServerError,
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

  pub async fn get_endpoint_manager_opt(&self) -> Option<EndpointManager> {
    let mg = self.inner.endpoint_manager.lock().await;
    mg.clone()
  }

  pub async fn get_endpoint_manager(&self) -> EndpointManager {
    let mg = self.inner.endpoint_manager.lock().await;
    mg.as_ref().expect("EndpointManager is not found").clone()
  }

  async fn set_endpoint_manager(&self, endpoint_manager: EndpointManager) {
    let mut mg = self.inner.endpoint_manager.lock().await;
    *mg = Some(endpoint_manager);
  }

  #[cfg(test)]
  pub async fn set_endpoint_manager_for_test(&self, endpoint_manager: EndpointManager) {
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

  pub async fn start(&self) -> Result<(), RemoteError> {
    self.start_with_callback(|| async {}).await
  }

  pub async fn start_with_callback<F, Fut>(&self, on_start: F) -> Result<(), RemoteError>
  where
    F: FnOnce() -> Fut,
    Fut: Future<Output = ()> + Send + Sync,
  {
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

    let mut process_registry = self.inner.actor_system.get_process_registry().await;
    process_registry
      .register_address_resolver(AddressResolver::new(move |pid| {
        let cloned_self = cloned_self.clone();
        let pid = pid.clone();
        async move {
          let (ph, _) = cloned_self.remote_handler(&pid.inner_pid).await;
          Some(ph)
        }
      }))
      .await;
    process_registry.set_address(published_address).await;
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
