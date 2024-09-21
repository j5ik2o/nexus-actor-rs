use crate::actor::actor::Props;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::{MessageHandle, ReadonlyMessageHeadersHandle};
use crate::actor::process::process_registry::AddressResolver;
use crate::actor::process::ProcessHandle;
use crate::extensions::{next_extension_id, Extension, ExtensionId};
use crate::generated::actor::Pid;
use crate::generated::remote::remoting_server::RemotingServer;
use crate::remote::block_list::BlockList;
use crate::remote::config::server_config::ServerConfig;
use crate::remote::config::Config;
use crate::remote::endpoint_manager::EndpointManager;
use crate::remote::endpoint_reader::EndpointReader;
use crate::remote::messages::RemoteDeliver;
use crate::remote::remote_process::RemoteProcess;
use crate::remote::serializer::SerializerId;
use dashmap::DashMap;
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

pub static EXTENSION_ID: Lazy<ExtensionId> = Lazy::new(|| next_extension_id());

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

#[derive(Debug, Clone)]
pub struct Remote {
  actor_system: ActorSystem,
  endpoint_reader: Arc<Mutex<Option<EndpointReader>>>,
  endpoint_manager: Arc<Mutex<Option<EndpointManager>>>,
  config: Config,
  kinds: Arc<DashMap<String, Props>>,
  block_list: BlockList,
  shutdown: Option<Shutdown>,
}

impl Remote {
  pub async fn new(actor_system: ActorSystem, config: Config) -> Self {
    let block_list = BlockList::new();
    let mut r = Remote {
      actor_system: actor_system.clone(),
      endpoint_reader: Arc::new(Mutex::new(None)),
      endpoint_manager: Arc::new(Mutex::new(None)),
      config: config.clone(),
      kinds: Arc::new(DashMap::new()),
      block_list,
      shutdown: None,
    };
    for (k, v) in config.get_kinds().await {
      r.register(&k, v);
    }
    actor_system
      .get_extensions()
      .await
      .register(Arc::new(Mutex::new(r.clone())))
      .await;
    r
  }

  async fn get_endpoint_reader(&self) -> EndpointReader {
    let mg = self.endpoint_reader.lock().await;
    mg.as_ref().expect("EndpointReader is not found").clone()
  }

  async fn set_endpoint_reader(&self, endpoint_reader: EndpointReader) {
    let mut mg = self.endpoint_reader.lock().await;
    *mg = Some(endpoint_reader);
  }

  pub async fn get_endpoint_manager_opt(&self) -> Option<EndpointManager> {
    let mg = self.endpoint_manager.lock().await;
    mg.clone()
  }

  pub async fn get_endpoint_manager(&self) -> EndpointManager {
    let mg = self.endpoint_manager.lock().await;
    mg.as_ref().expect("EndpointManager is not found").clone()
  }

  async fn set_endpoint_manager(&self, endpoint_manager: EndpointManager) {
    let mut mg = self.endpoint_manager.lock().await;
    *mg = Some(endpoint_manager);
  }

  pub fn get_config(&self) -> &Config {
    &self.config
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

  pub fn register(&mut self, kind: &str, props: Props) {
    self.kinds.insert(kind.to_string(), props);
  }

  pub fn get_known_kinds(&self) -> Vec<String> {
    self.kinds.iter().map(|kv| kv.key().clone()).collect()
  }

  pub async fn start(&mut self) -> Result<(), RemoteError> {
    self.start_with_callback(|| async {}).await
  }

  pub async fn start_with_callback<F, Fut>(&mut self, on_start: F) -> Result<(), RemoteError>
  where
    F: FnOnce() -> Fut,
    Fut: Future<Output = ()> + Send + Sync, {
    let (shutdown, rx) = Shutdown::new();
    self.shutdown = Some(shutdown);

    let my_self = Arc::new(self.clone());
    let cloned_self = my_self.clone();
    let mut server = Server::builder();
    if let Some(sc) = &self.config.get_server_config().await {
      Self::configure_server(server.clone(), sc);
    }

    let host_str = if let Some(advertise_host) = self.config.get_advertised_host().await {
      advertise_host
    } else {
      self.config.get_host().await.expect("Host is not found")
    };
    tracing::debug!("Host: {}", host_str);
    let port = self.config.get_port().await.expect("Port is not found");
    let socket_addrs = (host_str.clone(), port)
      .to_socket_addrs()
      .expect("Invalid host")
      .collect::<Vec<_>>();
    let socket_addr = socket_addrs
      .into_iter()
      .filter_map(|addr| if addr.is_ipv4() { Some(addr) } else { None })
      .next()
      .expect("Failed to resolve hostname");

    let mut process_registry = self.actor_system.get_process_registry().await;
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
    process_registry.set_address(format!("{}:{}", host_str, port)).await;
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
    if let Err(_) = router
      .serve_with_shutdown(socket_addr, async {
        tracing::info!("Server started: {}", socket_addr);
        on_start().await;
        rx.await.ok();
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

  async fn remote_handler(&self, pid: &Pid) -> (ProcessHandle, bool) {
    let ref_process = RemoteProcess::new(pid.clone(), self.clone());
    (ProcessHandle::new(ref_process.clone()), true)
  }

  pub async fn shutdown(&mut self, graceful: bool) -> Result<(), RemoteError> {
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
    if let Some(shutdown) = self.shutdown.take() {
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

#[cfg(test)]
mod tests {
  use crate::actor::actor::{Actor, ActorError, Props};
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{BasePart, ContextHandle, MessagePart, SenderPart, SpawnerPart};
  use crate::actor::message::Message;
  use crate::actor::message::{MessageHandle, ResponseHandle};
  use crate::actor::util::WaitGroup;

  use crate::remote::config::Config;
  use crate::remote::config_option::ConfigOption;

  use crate::remote::remote::Remote;
  use crate::remote::serializer::initialize_proto_serializers;
  use nexus_actor_message_derive_rs::Message;
  use std::env;
  use std::time::Duration;

  use tokio::time::sleep;

  use tracing_subscriber::EnvFilter;

  #[tokio::test]
  async fn test_start() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await.unwrap();
    let config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(8080)]).await;

    tracing::debug!("config: {:?}", config);
    let mut remote = Remote::new(system.clone(), config).await;
    let mut cloned_remote = remote.clone();
    tokio::spawn(async move {
      let result = cloned_remote.start().await;
      assert!(result.is_ok());
    });
    sleep(Duration::from_secs(3)).await;
    assert_eq!("127.0.0.1:8080", system.get_address().await);
    let result = remote.shutdown(true).await;
    assert!(result.is_ok());
    sleep(Duration::from_secs(1)).await;
  }

  #[tokio::test]
  async fn test_advertised_host() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await.unwrap();
    let config = Config::from([
      ConfigOption::with_advertised_host("localhost"),
      ConfigOption::with_port(8080),
    ])
    .await;

    tracing::debug!("config: {:?}", config);
    let mut remote = Remote::new(system.clone(), config).await;
    let mut cloned_remote = remote.clone();
    tokio::spawn(async move {
      let result = cloned_remote.start().await;
      assert!(result.is_ok());
    });
    sleep(Duration::from_secs(3)).await;
    assert_eq!("localhost:8080", system.get_address().await);
    let result = remote.shutdown(true).await;
    assert!(result.is_ok());
    sleep(Duration::from_secs(1)).await;
  }

  #[derive(Clone, PartialEq, Message, prost::Message)]
  pub struct EchoMessage {
    #[prost(string, tag = "1")]
    pub message: String,
  }

  impl EchoMessage {
    pub fn new(message: String) -> Self {
      Self { message }
    }
  }

  #[derive(Debug, Clone)]
  struct EchoActor;

  #[async_trait::async_trait]
  impl Actor for EchoActor {
    async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
      if let Some(msg) = ctx.get_message_handle().await.to_typed::<EchoMessage>() {
        tracing::info!(">>> EchoActor received: {}", msg.message);
        ctx
          .respond(ResponseHandle::new(EchoMessage::new(format!("Echo: {}", msg.message))))
          .await;
      }
      Ok(())
    }
  }

  #[tokio::test]
  async fn test_register() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await.unwrap();
    let config = Config::from([
      ConfigOption::with_host("127.0.0.1"),
      ConfigOption::with_port(8080),
      ConfigOption::with_kind("someKind", Props::from_actor_receiver(|_| async { Ok(()) }).await),
      ConfigOption::with_kind("someOther", Props::from_actor_receiver(|_| async { Ok(()) }).await),
    ])
    .await;

    tracing::debug!("config: {:?}", config);
    let remote = Remote::new(system, config).await;
    let mut kinds = remote.get_known_kinds();
    assert_eq!(2, kinds.len());
    kinds.sort();
    assert_eq!("someKind", kinds[0]);
    assert_eq!("someOther", kinds[1]);
  }

  #[tokio::test]
  async fn test_remote_communication() {
    let _ = env::set_var("RUST_LOG", "nexus_actor_core_rs=info");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    initialize_proto_serializers::<EchoMessage>().expect("Failed to register serializer");

    // サーバー側のセットアップ
    let server_wait_group = WaitGroup::with_count(1);
    let server_system = ActorSystem::new().await.unwrap();
    let server_config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(8090)]).await;
    let mut server_remote = Remote::new(server_system.clone(), server_config).await;
    let cloned_server_wait_group = server_wait_group.clone();
    tokio::spawn(async move {
      server_remote
        .start_with_callback(|| async {
          cloned_server_wait_group.done().await;
        })
        .await
        .expect("Failed to start server");
    });

    server_wait_group.wait().await;

    // エコーアクターをサーバーに登録して起動
    let echo_props = Props::from_actor_producer(|_| async { EchoActor }).await;
    let echo_pid = server_system
      .get_root_context()
      .await
      .spawn_named(echo_props, "echo")
      .await
      .unwrap();

    let client_wait_group = WaitGroup::with_count(1);
    let client_system = ActorSystem::new().await.unwrap();
    let client_config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(8091)]).await;
    let mut client_remote = Remote::new(client_system.clone(), client_config).await;
    let cloned_client_wait_group = client_wait_group.clone();
    tokio::spawn(async move {
      client_remote
        .start_with_callback(|| async {
          cloned_client_wait_group.done().await;
        })
        .await
        .expect("Failed to start client");
    });

    client_wait_group.wait().await;

    let root_context = client_system.get_root_context().await;

    let response = root_context
      .request_future(
        echo_pid,
        MessageHandle::new(EchoMessage::new("Hello, Remote!".to_string())),
        Duration::from_secs(10),
      )
      .await
      .result()
      .await
      .unwrap();

    if let Some(echo_response) = response.to_typed::<EchoMessage>() {
      tracing::info!("Received response: {}", echo_response.message);
      assert_eq!(echo_response.message, "Echo: Hello, Remote!");
    } else {
      panic!("Unexpected response type");
    }
  }
}
