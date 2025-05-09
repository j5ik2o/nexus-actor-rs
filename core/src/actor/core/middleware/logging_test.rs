#[cfg(test)]
mod test {
  use std::env;
  use std::sync::{Arc, Mutex};
  use std::time::Duration;

  use async_trait::async_trait;
  use tracing_subscriber::EnvFilter;

  use crate::actor::core::{ErrorReason, ExtendedPid};
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::message::MessageHandle;
  use crate::actor::core::middleware::logging::LoggingMiddleware;
  use crate::actor::core::middleware::{Middleware, MiddlewareChain};
  use crate::generated::actor::Pid;

  #[derive(Debug, Clone)]
  struct TestMessage(String);

  #[derive(Debug)]
  struct MockMiddlewareChain {
    last_action: Arc<Mutex<String>>,
    should_fail: Arc<Mutex<bool>>,
  }

  impl MockMiddlewareChain {
    fn new() -> Self {
      Self {
        last_action: Arc::new(Mutex::new(String::new())),
        should_fail: Arc::new(Mutex::new(false)),
      }
    }

    fn set_should_fail(&self, fail: bool) {
      *self.should_fail.lock().unwrap() = fail;
    }
  }

  #[async_trait]
  impl MiddlewareChain for MockMiddlewareChain {
    async fn handle(&self, msg: MessageHandle) -> Result<(), ErrorReason> {
      if *self.should_fail.lock().unwrap() {
        return Err(ErrorReason::new("test failure", 1));
      }
      *self.last_action.lock().unwrap() = format!("handled: {}", msg.message_type());
      Ok(())
    }
  }

  async fn setup_test_environment() -> (ActorSystem, LoggingMiddleware, MockMiddlewareChain) {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let actor_system = ActorSystem::new().await.unwrap();
    let logging_middleware = LoggingMiddleware::new();
    let mock_chain = MockMiddlewareChain::new();
    
    (actor_system, logging_middleware, mock_chain)
  }

  #[tokio::test]
  async fn test_logging_middleware_success() {
    let (_, logging_middleware, mock_chain) = setup_test_environment().await;
    let message = MessageHandle::new(TestMessage("test message".to_string()));

    let result = logging_middleware.handle(message.clone(), Arc::new(mock_chain.clone())).await;
    assert!(result.is_ok());

    let last_action = mock_chain.last_action.lock().unwrap().clone();
    assert_eq!(last_action, format!("handled: {}", message.message_type()));
  }

  #[tokio::test]
  async fn test_logging_middleware_failure() {
    let (_, logging_middleware, mock_chain) = setup_test_environment().await;
    let message = MessageHandle::new(TestMessage("test message".to_string()));

    mock_chain.set_should_fail(true);
    let result = logging_middleware.handle(message.clone(), Arc::new(mock_chain.clone())).await;

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert_eq!(error.code(), 1);
    assert_eq!(error.message(), "test failure");
  }

  #[tokio::test]
  async fn test_logging_middleware_chain() {
    let (_, logging_middleware1, mock_chain) = setup_test_environment().await;
    let logging_middleware2 = LoggingMiddleware::new();
    let message = MessageHandle::new(TestMessage("test message".to_string()));

    // Create a chain of two logging middlewares
    let result = logging_middleware1
      .handle(
        message.clone(),
        Arc::new(logging_middleware2.clone())
          .handle(message.clone(), Arc::new(mock_chain.clone()))
          .await
          .unwrap(),
      )
      .await;

    assert!(result.is_ok());
    let last_action = mock_chain.last_action.lock().unwrap().clone();
    assert_eq!(last_action, format!("handled: {}", message.message_type()));
  }
}
