use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{BasePart, ContextHandle, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, ExtendedPid, Props};
use nexus_actor_std_rs::actor::message::Message;
use nexus_actor_std_rs::actor::message::{MessageHandle, ResponseHandle};

use crate::config::Config;
use crate::config_option::ConfigOption;
use crate::endpoint_manager::EndpointManager;
use crate::endpoint_state::ConnectionState;
use crate::remote::{Remote, RemoteError, RemoteSpawnError};
use crate::response_status_code::ResponseStatusCode;
use crate::serializer::initialize_proto_serializers;
use crate::TransportEndpoint;
use nexus_message_derive_rs::Message;
use std::env;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::time::Duration;

use nexus_actor_core_rs::runtime::{CoreJoinHandle, CoreTaskFuture};
use tokio::sync::{oneshot, OnceCell};
use tokio::time::timeout;

use tracing_subscriber::EnvFilter;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

struct RemoteTaskHandle {
  handle: Arc<dyn CoreJoinHandle>,
  result: oneshot::Receiver<Result<(), RemoteError>>,
}

impl RemoteTaskHandle {
  async fn wait(self) -> Result<(), RemoteError> {
    let RemoteTaskHandle { handle, result } = self;
    let join_handle = handle.clone();
    let outcome = result.await.unwrap_or(Err(RemoteError::ServerError));
    join_handle.join().await;
    outcome
  }
}

struct RunningRemote {
  system: ActorSystem,
  remote: Arc<Remote>,
  handle: Option<RemoteTaskHandle>,
  socket_addr: SocketAddr,
}

impl RunningRemote {
  async fn start(options: Vec<ConfigOption>) -> Result<Self, Box<dyn std::error::Error>> {
    let system = ActorSystem::new()
      .await
      .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    let config = Config::from(options).await;
    let port = config.get_port().await.ok_or("port not set")?;
    let socket_addr = SocketAddr::new("127.0.0.1".parse()?, port);
    let remote = Arc::new(Remote::new(system.clone(), config).await);
    let handle = start_remote_instance(remote.clone()).await?;
    Ok(RunningRemote {
      system,
      remote,
      handle: Some(handle),
      socket_addr,
    })
  }

  async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error>> {
    self.remote.shutdown(true).await?;
    if let Some(handle) = self.handle.take() {
      handle.wait().await?;
    }
    Ok(())
  }

  async fn spawn_echo(&self, name: &str) -> Result<ExtendedPid, Box<dyn std::error::Error>> {
    let props = Props::from_async_actor_producer(|_| async { EchoActor }).await;
    let pid = self.system.get_root_context().await.spawn_named(props, name).await?;
    Ok(pid)
  }

  async fn endpoint_manager(&self) -> EndpointManager {
    self.remote.get_endpoint_manager().await
  }

  fn socket_addr(&self) -> std::net::SocketAddr {
    self.socket_addr
  }
}

fn allocate_port() -> Result<u16, Box<dyn std::error::Error>> {
  let listener = TcpListener::bind("127.0.0.1:0")?;
  let port = listener.local_addr()?.port();
  drop(listener);
  Ok(port)
}

fn server_options(port: u16) -> Vec<ConfigOption> {
  vec![ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(port)]
}

fn client_options(port: u16) -> Vec<ConfigOption> {
  vec![
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(port),
    ConfigOption::with_endpoint_reconnect_initial_backoff(Duration::from_millis(50)),
    ConfigOption::with_endpoint_reconnect_max_backoff(Duration::from_millis(200)),
    ConfigOption::with_endpoint_reconnect_max_retries(0),
  ]
}

fn spawn_remote_start(
  remote: Arc<Remote>,
) -> Result<(RemoteTaskHandle, oneshot::Receiver<()>), nexus_actor_core_rs::runtime::CoreSpawnError> {
  let spawner = remote.get_actor_system().core_runtime().spawner();
  let (result_tx, result_rx) = oneshot::channel();
  let (started_tx, started_rx) = oneshot::channel();
  let started_signal = Arc::new(tokio::sync::Mutex::new(Some(started_tx)));
  let started_for_callback = started_signal.clone();
  let started_for_failure = started_signal.clone();
  let remote_clone = remote.clone();

  let future: CoreTaskFuture = Box::pin(async move {
    let start_result = remote_clone
      .start_with_callback(move || {
        let started_for_callback = started_for_callback.clone();
        async move {
          if let Some(tx) = started_for_callback.lock().await.take() {
            let _ = tx.send(());
          }
        }
      })
      .await;

    if start_result.is_err() {
      if let Some(tx) = started_for_failure.lock().await.take() {
        let _ = tx.send(());
      }
    }

    let _ = result_tx.send(start_result);
  });

  let handle = spawner.spawn(future)?;
  Ok((
    RemoteTaskHandle {
      handle,
      result: result_rx,
    },
    started_rx,
  ))
}

async fn start_remote_instance(remote: Arc<Remote>) -> Result<RemoteTaskHandle, Box<dyn std::error::Error>> {
  let (handle, started_rx) = match spawn_remote_start(remote) {
    Ok(pair) => pair,
    Err(err) => {
      return Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        format!("failed to spawn remote task: {:?}", err),
      )) as Box<dyn std::error::Error>);
    }
  };

  match timeout(Duration::from_secs(5), started_rx).await {
    Ok(Ok(())) => Ok(handle),
    Ok(Err(_)) | Err(_) => {
      let result = handle.wait().await;
      match result {
        Ok(()) => Err("remote start did not complete successfully".into()),
        Err(err) => Err(Box::new(err)),
      }
    }
  }
}

#[tokio::test]
async fn test_start() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let port = allocate_port().unwrap();
  let mut running = RunningRemote::start(server_options(port)).await.unwrap();
  assert_eq!(format!("127.0.0.1:{}", port), running.system.get_address().await);
  running.shutdown().await.unwrap();
}

#[tokio::test]
async fn test_advertised_address() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let port = allocate_port().unwrap();
  let mut running = RunningRemote::start(vec![
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(port),
    ConfigOption::with_transport_endpoint(TransportEndpoint::new("localhost:6500".to_string())),
  ])
  .await
  .unwrap();
  assert_eq!("localhost:6500", running.system.get_address().await);
  running.shutdown().await.unwrap();
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
    if let Some(msg) = ctx
      .get_message_handle_opt()
      .await
      .expect("message not found")
      .to_typed::<EchoMessage>()
    {
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
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await.unwrap();
  let config = Config::from([
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(8080),
    ConfigOption::with_kind("someKind", Props::from_async_actor_receiver(|_| async { Ok(()) }).await),
    ConfigOption::with_kind(
      "someOther",
      Props::from_async_actor_receiver(|_| async { Ok(()) }).await,
    ),
  ])
  .await;

  tracing::debug!("config: {:?}", config);
  let remote = Arc::new(Remote::new(system, config).await);
  let mut kinds = remote.get_known_kinds();
  assert_eq!(2, kinds.len());
  kinds.sort();
  assert_eq!("someKind", kinds[0]);
  assert_eq!("someOther", kinds[1]);
}

#[tokio::test]
async fn test_remote_communication() {
  env::set_var("RUST_LOG", "nexus_actor_std_rs=info");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  initialize_proto_serializers::<EchoMessage>().expect("Failed to register serializer");

  // サーバー側のセットアップ
  let server_system = ActorSystem::new().await.unwrap();
  let server_config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(8090)]).await;
  let server_remote = Arc::new(Remote::new(server_system.clone(), server_config).await);
  let echo_props = Props::from_async_actor_producer(|_| async { EchoActor }).await;
  let echo_kind = "echo-kind";
  server_remote.register(echo_kind, echo_props);

  let (server_runner, server_started) = spawn_remote_start(server_remote.clone()).expect("spawn server remote");
  timeout(Duration::from_secs(5), server_started)
    .await
    .expect("server start wait timed out")
    .expect("server start signal dropped");

  let client_system = ActorSystem::new().await.unwrap();
  let client_config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(8091)]).await;
  let client_remote = Arc::new(Remote::new(client_system.clone(), client_config).await);
  let (client_runner, client_started) = spawn_remote_start(client_remote.clone()).expect("spawn client remote");
  timeout(Duration::from_secs(5), client_started)
    .await
    .expect("client start wait timed out")
    .expect("client start signal dropped");

  let root_context = client_system.get_root_context().await;

  let echo_pid = client_remote
    .spawn_remote_named("127.0.0.1:8090", "echo", echo_kind, Duration::from_secs(5))
    .await
    .expect("remote spawn");

  let response = root_context
    .request_future(
      ExtendedPid::new(echo_pid.clone()),
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

  client_remote.shutdown(true).await.expect("client shutdown");
  client_runner.wait().await.expect("client runner");
  server_remote.shutdown(true).await.expect("server shutdown");
  server_runner.wait().await.expect("server runner");
}

#[tokio::test]
async fn spawn_remote_unknown_kind_returns_error() -> TestResult<()> {
  let server_port = allocate_port()?;
  let client_port = allocate_port()?;

  let server_system = ActorSystem::new().await.expect("server actor system");
  let server_config = Config::from([
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(server_port),
  ])
  .await;
  let server_remote = Arc::new(Remote::new(server_system, server_config).await);
  let (server_runner, server_started) = spawn_remote_start(server_remote.clone()).expect("spawn server");
  timeout(Duration::from_secs(5), server_started)
    .await
    .expect("server start wait timed out")
    .expect("server start signal dropped");

  let client_system = ActorSystem::new().await.expect("client actor system");
  let client_config = Config::from([
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(client_port),
  ])
  .await;
  let client_remote = Arc::new(Remote::new(client_system, client_config).await);
  let (client_runner, client_started) = spawn_remote_start(client_remote.clone()).expect("spawn client");
  timeout(Duration::from_secs(5), client_started)
    .await
    .expect("client start wait timed out")
    .expect("client start signal dropped");

  let result = client_remote
    .spawn_remote(
      &format!("127.0.0.1:{}", server_port),
      "missing-kind",
      Duration::from_secs(2),
    )
    .await;
  match result {
    Err(RemoteSpawnError::Status(err)) => {
      assert_eq!(err.code(), ResponseStatusCode::Error);
    }
    other => panic!("unexpected result: {other:?}"),
  }

  client_remote.shutdown(true).await?;
  client_runner.wait().await?;
  server_remote.shutdown(true).await?;
  server_runner.wait().await?;

  Ok(())
}

#[tokio::test]
async fn remote_block_list_public_api_works() -> TestResult<()> {
  let system = ActorSystem::new().await.expect("actor system init");
  let config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(0)]).await;
  let remote = Arc::new(Remote::new(system, config).await);

  // initially empty
  assert!(remote.list_blocked_systems().await.is_empty());

  remote.block_system("blocked-A").await;
  remote.block_system("blocked-B").await;
  let mut listed = remote.list_blocked_systems().await;
  listed.sort();
  assert_eq!(listed, vec!["blocked-A".to_string(), "blocked-B".to_string()]);

  // unblock one and verify
  remote.unblock_system("blocked-A").await;
  let listed = remote.list_blocked_systems().await;
  assert_eq!(listed, vec!["blocked-B".to_string()]);

  Ok(())
}

#[tokio::test]
async fn remote_reconnect_after_server_restart() -> Result<(), Box<dyn std::error::Error>> {
  static INIT: OnceCell<()> = OnceCell::const_new();
  let _ = INIT
    .get_or_init(|| async {
      let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();
    })
    .await;

  initialize_proto_serializers::<EchoMessage>().expect("Failed to register serializer");

  let server_port = allocate_port()?;
  let client_port = allocate_port()?;

  let mut server = RunningRemote::start(server_options(server_port)).await?;
  let mut echo_pid = server.spawn_echo("echo-reconnect").await?;

  let mut client = RunningRemote::start(client_options(client_port)).await?;

  let initial = client
    .system
    .get_root_context()
    .await
    .request_future(
      echo_pid.clone(),
      MessageHandle::new(EchoMessage::new("first".to_string())),
      Duration::from_secs(3),
    )
    .await
    .result()
    .await?;
  let initial_response = initial
    .to_typed::<EchoMessage>()
    .ok_or_else(|| "unexpected response".to_string())?;
  assert_eq!(initial_response.message, "Echo: first");

  let server_address = server.socket_addr();
  let server_address_str = server_address.to_string();
  let manager = client.endpoint_manager().await;

  server.shutdown().await?;

  manager.schedule_reconnect(server_address_str.clone()).await;

  server = RunningRemote::start(server_options(server_port)).await?;
  echo_pid = server.spawn_echo("echo-reconnect").await?;

  let manager_clone = manager.clone();
  let address_clone = server_address_str.clone();
  let state = timeout(Duration::from_secs(5), async move {
    manager_clone.await_reconnect(&address_clone).await
  })
  .await
  .map_err(|_| "reconnect timeout".to_string())?;
  assert_eq!(state, ConnectionState::Connected);

  let retry = client
    .system
    .get_root_context()
    .await
    .request_future(
      echo_pid.clone(),
      MessageHandle::new(EchoMessage::new("second".to_string())),
      Duration::from_secs(3),
    )
    .await
    .result()
    .await?;
  let retry_response = retry
    .to_typed::<EchoMessage>()
    .ok_or_else(|| "unexpected response".to_string())?;
  assert_eq!(retry_response.message, "Echo: second");

  if let Some(stats) = client.remote.get_endpoint_statistics(&server_address_str).await {
    assert!(stats.reconnect_attempts >= 1);
    assert!(stats.deliver_success >= 1);
  }

  client.shutdown().await?;
  server.shutdown().await?;

  Ok(())
}

#[tokio::test]
async fn spawn_remote_named_duplicate_returns_conflict() -> TestResult<()> {
  initialize_proto_serializers::<EchoMessage>().expect("Failed to register serializer");

  let server_port = allocate_port()?;
  let client_port = allocate_port()?;

  // server setup
  let server_system = ActorSystem::new().await.expect("server actor system");
  let server_config = Config::from([
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(server_port),
  ])
  .await;
  let server_remote = Arc::new(Remote::new(server_system.clone(), server_config).await);
  let echo_props = Props::from_async_actor_producer(|_| async { EchoActor }).await;
  let kind = "dup-kind";
  server_remote.register(kind, echo_props);

  let (server_runner, server_started) = spawn_remote_start(server_remote.clone()).expect("spawn server");
  timeout(Duration::from_secs(5), server_started)
    .await
    .expect("server start wait timed out")
    .expect("server start signal dropped");

  // client setup
  let client_system = ActorSystem::new().await.expect("client actor system");
  let client_config = Config::from([
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(client_port),
  ])
  .await;
  let client_remote = Arc::new(Remote::new(client_system.clone(), client_config).await);
  let (client_runner, client_started) = spawn_remote_start(client_remote.clone()).expect("spawn client");
  timeout(Duration::from_secs(5), client_started)
    .await
    .expect("client start wait timed out")
    .expect("client start signal dropped");

  let server_address = format!("127.0.0.1:{}", server_port);
  let name = "dup-actor";

  // first spawn succeeds
  client_remote
    .spawn_remote_named(&server_address, name, kind, Duration::from_secs(2))
    .await
    .expect("first remote spawn");

  // second spawn should conflict
  let result = client_remote
    .spawn_remote_named(&server_address, name, kind, Duration::from_secs(2))
    .await;
  match result {
    Err(RemoteSpawnError::Status(status)) => {
      assert_eq!(status.code(), ResponseStatusCode::ProcessNameAlreadyExists);
    }
    other => panic!("unexpected result: {other:?}"),
  }

  client_remote.shutdown(true).await?;
  client_runner.wait().await?;
  server_remote.shutdown(true).await?;
  server_runner.wait().await?;

  Ok(())
}
