use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{BasePart, ContextHandle, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_core_rs::actor::core::{Actor, ActorError, ExtendedPid, Props};
use nexus_actor_core_rs::actor::message::Message;
use nexus_actor_core_rs::actor::message::{MessageHandle, ResponseHandle};

use crate::config::Config;
use crate::config_option::ConfigOption;
use crate::endpoint_manager::EndpointManager;
use crate::endpoint_state::ConnectionState;
use crate::remote::{Remote, RemoteError, RemoteSpawnError};
use crate::response_status_code::ResponseStatusCode;
use crate::serializer::initialize_proto_serializers;
use nexus_actor_message_derive_rs::Message;
use std::env;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::OnceCell;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use nexus_actor_utils_rs::concurrent::WaitGroup;
use tracing_subscriber::EnvFilter;

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

struct RunningRemote {
  system: ActorSystem,
  remote: Arc<Remote>,
  handle: Option<JoinHandle<Result<(), RemoteError>>>,
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
      handle.await??;
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

async fn start_remote_instance(
  remote: Arc<Remote>,
) -> Result<JoinHandle<Result<(), RemoteError>>, Box<dyn std::error::Error>> {
  let wait_group = WaitGroup::with_count(1);
  let cloned_wait = wait_group.clone();
  let cloned_remote = remote.clone();

  let handle = tokio::spawn(async move {
    cloned_remote
      .start_with_callback(|| async {
        cloned_wait.done().await;
      })
      .await
  });

  if timeout(Duration::from_secs(5), wait_group.wait()).await.is_err() {
    let join_result = handle.await;
    return match join_result {
      Ok(Ok(())) => Err("remote start timed out".into()),
      Ok(Err(err)) => Err(Box::new(err)),
      Err(join_err) => Err(Box::new(join_err)),
    };
  }

  Ok(handle)
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
    ConfigOption::with_advertised_address("localhost:6500"),
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
    if let Some(msg) = ctx.get_message_handle_opt().await.expect("message not found").to_typed::<EchoMessage>() {
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
  let remote = Remote::new(system, config).await;
  let mut kinds = remote.get_known_kinds();
  assert_eq!(2, kinds.len());
  kinds.sort();
  assert_eq!("someKind", kinds[0]);
  assert_eq!("someOther", kinds[1]);
}

#[tokio::test]
async fn test_remote_communication() {
  env::set_var("RUST_LOG", "nexus_actor_core_rs=info");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  initialize_proto_serializers::<EchoMessage>().expect("Failed to register serializer");

  // サーバー側のセットアップ
  let server_wait_group = WaitGroup::with_count(1);
  let server_system = ActorSystem::new().await.unwrap();
  let server_config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(8090)]).await;
  let server_remote = Remote::new(server_system.clone(), server_config).await;
  let echo_props = Props::from_async_actor_producer(|_| async { EchoActor }).await;
  let echo_kind = "echo-kind";
  server_remote.register(echo_kind, echo_props);

  let cloned_server_wait_group = server_wait_group.clone();
  let server_remote_for_start = server_remote.clone();

  tokio::spawn(async move {
    server_remote_for_start
      .start_with_callback(|| async {
        cloned_server_wait_group.done().await;
      })
      .await
      .expect("Failed to start server");
  });

  server_wait_group.wait().await;

  let client_wait_group = WaitGroup::with_count(1);
  let client_system = ActorSystem::new().await.unwrap();
  let client_config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(8091)]).await;
  let client_remote = Remote::new(client_system.clone(), client_config).await;
  let cloned_client_wait_group = client_wait_group.clone();

  let client_remote_for_start = client_remote.clone();

  tokio::spawn(async move {
    client_remote_for_start
      .start_with_callback(|| async {
        cloned_client_wait_group.done().await;
      })
      .await
      .expect("Failed to start client");
  });

  client_wait_group.wait().await;

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
  let server_remote = Remote::new(server_system, server_config).await;
  let server_wait = WaitGroup::with_count(1);
  let server_wait_clone = server_wait.clone();
  let server_remote_start = server_remote.clone();
  tokio::spawn(async move {
    server_remote_start
      .start_with_callback(|| async {
        server_wait_clone.done().await;
      })
      .await
      .expect("server start");
  });
  server_wait.wait().await;

  let client_system = ActorSystem::new().await.expect("client actor system");
  let client_config = Config::from([
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(client_port),
  ])
  .await;
  let client_remote = Remote::new(client_system, client_config).await;
  let client_wait = WaitGroup::with_count(1);
  let client_wait_clone = client_wait.clone();
  let client_remote_start = client_remote.clone();
  tokio::spawn(async move {
    client_remote_start
      .start_with_callback(|| async {
        client_wait_clone.done().await;
      })
      .await
      .expect("client start");
  });
  client_wait.wait().await;

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
