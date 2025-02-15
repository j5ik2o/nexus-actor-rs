#[cfg(test)]
mod test {
  use std::env;
  use std::sync::{Arc, Mutex};
  use std::time::Duration;
  use std::net::SocketAddr;

  use async_trait::async_trait;
  use tracing_subscriber::EnvFilter;
  use tokio::sync::mpsc;

  use crate::remote::Remote;
  use crate::config::Config;
  use crate::messages::{RemoteMessage, RemoteResponse};
  use crate::remote_process::RemoteProcess;
  use crate::activator_actor::Activator;
  use crate::endpoint::Endpoint;
  use crate::generated::actor::Pid;
  use crate::actor::actor::{ErrorReason, ExtendedPid};
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::message::MessageHandle;

  #[derive(Debug, Clone)]
  struct TestMessage(String);

  #[derive(Debug)]
  struct MockRemoteActor {
    last_message: Arc<Mutex<Option<String>>>,
    should_fail: Arc<Mutex<bool>>,
  }

  impl MockRemoteActor {
    fn new() -> Self {
      Self {
        last_message: Arc::new(Mutex::new(None)),
        should_fail: Arc::new(Mutex::new(false)),
      }
    }

    fn set_should_fail(&self, fail: bool) {
      *self.should_fail.lock().unwrap() = fail;
    }
  }

  async fn setup_test_environment() -> (ActorSystem, Remote, MockRemoteActor) {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let actor_system = ActorSystem::new().await.unwrap();
    let config = Config::new("127.0.0.1:0".parse::<SocketAddr>().unwrap());
    let remote = Remote::new(config);
    let mock_actor = MockRemoteActor::new();
    
    (actor_system, remote, mock_actor)
  }

  #[tokio::test]
  async fn test_remote_message_serialization() {
    let (actor_system, remote, _) = setup_test_environment().await;
    let message = TestMessage("test message".to_string());
    let remote_message = RemoteMessage::new(
      Pid::new("test", "1"),
      MessageHandle::new(message.clone()),
    );

    // Test serialization
    let serialized = remote_message.serialize().await.unwrap();
    let deserialized = RemoteMessage::deserialize(&serialized).await.unwrap();

    assert_eq!(deserialized.target().id(), "1");
    assert_eq!(deserialized.target().address(), "test");
  }

  #[tokio::test]
  async fn test_remote_process_creation() {
    let (actor_system, remote, mock_actor) = setup_test_environment().await;
    let pid = Pid::new("test", "1");
    let process = RemoteProcess::new(pid.clone(), remote.clone());

    assert_eq!(process.get_pid().id(), pid.id());
    assert_eq!(process.get_pid().address(), pid.address());
  }

  #[tokio::test]
  async fn test_endpoint_connection() {
    let (actor_system, remote, mock_actor) = setup_test_environment().await;
    let endpoint = Endpoint::new("127.0.0.1:0".parse::<SocketAddr>().unwrap());
    
    assert_eq!(endpoint.get_address(), "127.0.0.1:0");
    assert!(endpoint.is_connected().await);
  }

  #[tokio::test]
  async fn test_activator_spawn() {
    let (actor_system, remote, mock_actor) = setup_test_environment().await;
    let activator = Activator::new(actor_system.clone());
    let address = "test_actor";
    let name = "test1";

    let pid = activator.spawn_named(
      address,
      name,
      "TestActor",
      Duration::from_secs(5),
    ).await.unwrap();

    assert_eq!(pid.address(), address);
    assert_eq!(pid.id(), name);
  }
}
