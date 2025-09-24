use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{BasePart, ContextHandle, MessagePart, SenderPart, SpawnerPart};
use nexus_actor_core_rs::actor::core::{Actor, ActorError, Props};
use nexus_actor_core_rs::actor::message::Message;
use nexus_actor_core_rs::actor::message::{MessageHandle, ResponseHandle};

use crate::config::Config;
use crate::config_option::ConfigOption;

use crate::remote::Remote;
use crate::serializer::initialize_proto_serializers;
use nexus_actor_message_derive_rs::Message;
use std::env;
use std::time::Duration;

use tokio::time::sleep;

use nexus_actor_utils_rs::concurrent::WaitGroup;
use tracing_subscriber::EnvFilter;

#[tokio::test]
async fn test_start() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await.unwrap();
  let config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(8080)]).await;

  tracing::debug!("config: {:?}", config);
  let remote = Remote::new(system.clone(), config).await;
  let cloned_remote = remote.clone();
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
async fn test_advertised_address() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await.unwrap();
  let config = Config::from([
    ConfigOption::with_host("127.0.0.1"),
    ConfigOption::with_port(8080),
    ConfigOption::with_advertised_address("localhost:6500"),
  ])
  .await;

  tracing::debug!("config: {:?}", config);
  let remote = Remote::new(system.clone(), config).await;
  let cloned_remote = remote.clone();
  tokio::spawn(async move {
    let result = cloned_remote.start().await;
    assert!(result.is_ok());
  });
  sleep(Duration::from_secs(3)).await;
  assert_eq!("localhost:6500", system.get_address().await);
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
  let echo_props = Props::from_async_actor_producer(|_| async { EchoActor }).await;
  let echo_pid = server_system
    .get_root_context()
    .await
    .spawn_named(echo_props, "echo")
    .await
    .unwrap();

  let client_wait_group = WaitGroup::with_count(1);
  let client_system = ActorSystem::new().await.unwrap();
  let client_config = Config::from([ConfigOption::with_host("127.0.0.1"), ConfigOption::with_port(8091)]).await;
  let client_remote = Remote::new(client_system.clone(), client_config).await;
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
