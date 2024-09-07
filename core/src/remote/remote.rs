use crate::actor::actor::Props;
use crate::actor::actor_system::ActorSystem;
use crate::extensions::{next_extension_id, Extension, ExtensionId};
use crate::generated::actor::Pid;
use crate::generated::remote::remoting_server::RemotingServer;
use crate::remote::block_list::BlockList;
use crate::remote::config::server_config::ServerConfig;
use crate::remote::config::Config;
use crate::remote::endpoint_manager::EndpointManager;
use crate::remote::endpoint_reader::EndpointReader;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::any::Any;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot::Sender;
use tokio::sync::{oneshot, Mutex};
use tonic::transport::Server;

pub enum RemoteError {
  ServerError,
}

pub static EXTENSION_ID: Lazy<ExtensionId> = Lazy::new(|| next_extension_id());

#[derive(Debug)]
pub struct Shutdown {
  tx: Option<oneshot::Sender<()>>,
}

impl Shutdown {
  pub fn new() -> (Self, oneshot::Receiver<()>) {
    let (tx, rx) = oneshot::channel();
    (Shutdown { tx: Some(tx) }, rx)
  }

  pub async fn shutdown(&mut self) {
    if let Some(tx) = self.tx.take() {
      let _ = tx.send(());
    }
  }
}

#[derive(Debug, Clone)]
pub struct Remote {
  actor_system: ActorSystem,
  endpoint_reader: Option<Arc<EndpointReader>>,
  endpoint_manager: Option<Arc<EndpointManager>>,
  config: Config,
  kinds: DashMap<String, Props>,
  activator_pid: Option<Pid>,
  block_list: BlockList,
  shutdown: Option<Arc<Mutex<Shutdown>>>,
}

impl Remote {
  pub fn new(actor_system: ActorSystem, config: Config) -> Self {
    let block_list = BlockList::new();
    Remote {
      actor_system,
      endpoint_reader: None,
      endpoint_manager: None,
      config,
      kinds: DashMap::new(),
      activator_pid: None,
      block_list,
      shutdown: None,
    }
  }

  pub fn get_kinds(&self) -> &DashMap<String, Props> {
    &self.kinds
  }

  pub fn get_actor_system(&self) -> &ActorSystem {
    &self.actor_system
  }

  pub fn get_block_list(&self) -> &BlockList {
    &self.block_list
  }

  pub fn get_block_list_mut(&mut self) -> &mut BlockList {
    &mut self.block_list
  }

  pub async fn start(&mut self) -> Result<(), RemoteError> {
    let (shutdown, rx) = Shutdown::new();
    self.shutdown = Some(Arc::new(Mutex::new(shutdown)));
    let my_self = Arc::new(Mutex::new(self.clone()));
    let mut server = Server::builder();
    if let Some(sc) = &self.config.server_config {
      Self::configure_server(server.clone(), sc);
    }
    let router = server.add_service(RemotingServer::new(EndpointReader::new(my_self)));
    if let Err(_) = router
      .serve_with_shutdown(self.config.get_socket_address(), async {
        rx.await.ok();
        tracing::info!("Shutting down server");
      })
      .await
    {
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

  pub async fn stop(&mut self) {
    if let Some(shutdown) = self.shutdown.take() {
      let mut mg = shutdown.lock().await;
      mg.shutdown().await;
    }
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
