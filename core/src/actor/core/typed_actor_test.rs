#[cfg(test)]
mod test {
  use std::env;
  use std::sync::{Arc, Mutex};
  use std::time::Duration;

  use async_trait::async_trait;
  use tracing_subscriber::EnvFilter;

  use crate::actor::core::{ErrorReason, ExtendedPid, RestartStatistics};
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::message::MessageHandle;
  use crate::actor::core::{TypedActor, TypedActorHandle, TypedProps};
  use crate::generated::actor::Pid;

  // Test message types
  #[derive(Debug, Clone)]
  struct TestMessage(String);

  #[derive(Debug, Clone)]
  struct TestResponse(String);

  // Mock typed actor for testing
  #[derive(Debug)]
  struct MockTypedActor {
    last_message: Arc<Mutex<Option<String>>>,
    should_fail: Arc<Mutex<bool>>,
  }

  impl MockTypedActor {
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

  #[async_trait]
  impl TypedActor for MockTypedActor {
    type Message = TestMessage;
    type Response = TestResponse;

    async fn handle_message(&self, msg: Self::Message) -> Result<Self::Response, ErrorReason> {
      if *self.should_fail.lock().unwrap() {
        return Err(ErrorReason::new("test failure", 1));
      }
      *self.last_message.lock().unwrap() = Some(msg.0.clone());
      Ok(TestResponse(format!("processed: {}", msg.0)))
    }
  }

  async fn setup_test_environment() -> (ActorSystem, MockTypedActor, TypedProps<MockTypedActor>) {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let actor_system = ActorSystem::new().await.unwrap();
    let mock_actor = MockTypedActor::new();
    let props = TypedProps::new(mock_actor.clone());
    
    (actor_system, mock_actor, props)
  }

  #[tokio::test]
  async fn test_typed_actor_handle_message_success() {
    let (actor_system, mock_actor, props) = setup_test_environment().await;
    let handle = TypedActorHandle::new(props, actor_system);

    let message = TestMessage("test message".to_string());
    let response = handle.send(message.clone()).await.unwrap();

    match response {
      TestResponse(msg) => assert_eq!(msg, format!("processed: {}", message.0)),
    }

    let last_message = mock_actor.last_message.lock().unwrap();
    assert_eq!(last_message.as_ref().unwrap(), &message.0);
  }

  #[tokio::test]
  async fn test_typed_actor_handle_message_failure() {
    let (actor_system, mock_actor, props) = setup_test_environment().await;
    let handle = TypedActorHandle::new(props, actor_system);

    mock_actor.set_should_fail(true);
    let message = TestMessage("test message".to_string());
    let result = handle.send(message).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.code(), 1);
    assert_eq!(error.message(), "test failure");
  }

  #[tokio::test]
  async fn test_typed_actor_restart() {
    let (actor_system, mock_actor, props) = setup_test_environment().await;
    let handle = TypedActorHandle::new(props, actor_system.clone());

    // First message succeeds
    let message1 = TestMessage("first message".to_string());
    let response1 = handle.send(message1.clone()).await.unwrap();
    assert_eq!(response1.0, format!("processed: {}", message1.0));

    // Set to fail and send message
    mock_actor.set_should_fail(true);
    let message2 = TestMessage("failing message".to_string());
    let result = handle.send(message2).await;
    assert!(result.is_err());

    // Reset failure flag (simulating restart) and try again
    mock_actor.set_should_fail(false);
    let message3 = TestMessage("after failure".to_string());
    let response3 = handle.send(message3.clone()).await.unwrap();
    assert_eq!(response3.0, format!("processed: {}", message3.0));
  }
}
