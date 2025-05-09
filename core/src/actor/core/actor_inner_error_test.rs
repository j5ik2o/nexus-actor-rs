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
  use crate::actor::core::actor_inner_error::ActorInnerError;
  use crate::generated::actor::Pid;

  #[derive(Debug, Clone)]
  struct TestMessage(String);

  #[derive(Debug)]
  struct MockActor {
    last_error: Arc<Mutex<Option<ActorInnerError>>>,
    should_fail: Arc<Mutex<bool>>,
  }

  impl MockActor {
    fn new() -> Self {
      Self {
        last_error: Arc::new(Mutex::new(None)),
        should_fail: Arc::new(Mutex::new(false)),
      }
    }

    fn set_should_fail(&self, fail: bool) {
      *self.should_fail.lock().unwrap() = fail;
    }

    fn get_last_error(&self) -> Option<ActorInnerError> {
      self.last_error.lock().unwrap().clone()
    }
  }

  async fn setup_test_environment() -> (ActorSystem, MockActor) {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let actor_system = ActorSystem::new().await.unwrap();
    let mock_actor = MockActor::new();
    
    (actor_system, mock_actor)
  }

  #[tokio::test]
  async fn test_actor_inner_error_creation() {
    let (_, mock_actor) = setup_test_environment().await;
    let error_reason = ErrorReason::new("test error", 1);
    let message = MessageHandle::new(TestMessage("test message".to_string()));

    let inner_error = ActorInnerError::new(error_reason.clone(), message.clone());
    *mock_actor.last_error.lock().unwrap() = Some(inner_error);

    let last_error = mock_actor.get_last_error().unwrap();
    assert_eq!(last_error.reason().code(), 1);
    assert_eq!(last_error.reason().message(), "test error");
    assert_eq!(last_error.message().message_type(), message.message_type());
  }

  #[tokio::test]
  async fn test_actor_inner_error_propagation() {
    let (_, mock_actor) = setup_test_environment().await;
    let error_reason = ErrorReason::new("propagated error", 2);
    let message = MessageHandle::new(TestMessage("test message".to_string()));

    let inner_error = ActorInnerError::new(error_reason.clone(), message.clone());
    let propagated_error = inner_error.with_reason(ErrorReason::new("new error", 3));
    
    assert_eq!(propagated_error.reason().code(), 3);
    assert_eq!(propagated_error.reason().message(), "new error");
    assert_eq!(propagated_error.message().message_type(), message.message_type());
  }

  #[tokio::test]
  async fn test_actor_inner_error_debug_display() {
    let (_, mock_actor) = setup_test_environment().await;
    let error_reason = ErrorReason::new("display test", 4);
    let message = MessageHandle::new(TestMessage("test message".to_string()));

    let inner_error = ActorInnerError::new(error_reason.clone(), message.clone());
    let debug_str = format!("{:?}", inner_error);
    let display_str = format!("{}", inner_error);

    assert!(debug_str.contains("display test"));
    assert!(debug_str.contains("4"));
    assert!(display_str.contains("display test"));
    assert!(display_str.contains("4"));
  }
}
