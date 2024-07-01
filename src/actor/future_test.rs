use super::*;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::future::{Future, FutureError, FutureProcess};
use crate::actor::message::{Message, MessageHandle};
use crate::actor::process::Process;
use async_trait::async_trait;
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[derive(Debug)]
struct MockProcess {
  name: String,
  received: Arc<AtomicBool>,
}

#[async_trait]
impl Process for MockProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, message: MessageHandle) {
    println!("MockProcess {} received message", self.name); // デバッグログ
    self.received.store(true, Ordering::SeqCst);
  }

  async fn send_system_message(&self, _: &ExtendedPid, _: MessageHandle) {}

  async fn stop(&self, _: &ExtendedPid) {}

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[tokio::test]
async fn test_future_pipe_to_message() {
  let a1 = Arc::new(MockProcess {
    name: "a1".to_string(),
    received: Arc::new(AtomicBool::new(false)),
  });
  let a2 = Arc::new(MockProcess {
    name: "a2".to_string(),
    received: Arc::new(AtomicBool::new(false)),
  });
  let a3 = Arc::new(MockProcess {
    name: "a3".to_string(),
    received: Arc::new(AtomicBool::new(false)),
  });

  let f = FutureProcess::new(Duration::from_secs(1));

  f.pipe_to(a1.clone()).await;
  f.pipe_to(a2.clone()).await;
  f.pipe_to(a3.clone()).await;

  f.send_user_message(None, MessageHandle::new("hello".to_string())).await;

  // 非同期処理の完了を待つ
  tokio::time::sleep(Duration::from_millis(100)).await;

  assert!(a1.received.load(Ordering::SeqCst));
  assert!(a2.received.load(Ordering::SeqCst));
  assert!(a3.received.load(Ordering::SeqCst));

  assert!(!f.is_empty().await, "future should not be empty after completion");
}

#[tokio::test]
async fn test_future_pipe_to_timeout_sends_error() {
  let a1 = Arc::new(MockProcess {
    name: "a1".to_string(),
    received: Arc::new(AtomicBool::new(false)),
  });
  let a2 = Arc::new(MockProcess {
    name: "a2".to_string(),
    received: Arc::new(AtomicBool::new(false)),
  });
  let a3 = Arc::new(MockProcess {
    name: "a3".to_string(),
    received: Arc::new(AtomicBool::new(false)),
  });

  let f = FutureProcess::new(Duration::from_millis(100));

  f.pipe_to(a1.clone()).await;
  f.pipe_to(a2.clone()).await;
  f.pipe_to(a3.clone()).await;

  // タイムアウトを待つ
  tokio::time::sleep(Duration::from_millis(200)).await;

  let err = f.result().await;
  assert!(err.is_err());
  assert!(matches!(err.unwrap_err(), FutureError::Timeout));

  // パイプ処理の完了を待つ
  for _ in 0..10 {
    if a1.received.load(Ordering::SeqCst) && a2.received.load(Ordering::SeqCst) && a3.received.load(Ordering::SeqCst) {
      break;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
  }

  assert!(a1.received.load(Ordering::SeqCst), "a1 did not receive message");
  assert!(a2.received.load(Ordering::SeqCst), "a2 did not receive message");
  assert!(a3.received.load(Ordering::SeqCst), "a3 did not receive message");

  assert!(!f.is_empty().await, "future should not be empty after timeout");
}

#[tokio::test]
async fn test_new_future_timeout_no_race() {
  let future = FutureProcess::new(Duration::from_millis(200));

  tokio::spawn({
    let future = future.clone();
    async move {
      sleep(Duration::from_millis(100)).await;
      future.complete(MessageHandle::new("response".to_string())).await;
    }
  });

  // 完了を待つ
  tokio::time::sleep(Duration::from_millis(150)).await;

  let result = future.result().await;
  assert!(result.is_ok(), "Expected Ok, got {:?}", result);
}

async fn assert_future_success(future: &FutureProcess) -> MessageHandle {
  match future.result().await {
    Ok(res) => res,
    Err(e) => panic!("Future failed: {:?}", e),
  }
}

#[tokio::test]
async fn test_future_result_dead_letter_response() {
  let future = FutureProcess::new(Duration::from_secs(1));
  future.fail(FutureError::DeadLetter).await;

  let result = future.result().await;
  assert!(matches!(result.unwrap_err(), FutureError::DeadLetter));
}

#[tokio::test]
async fn test_future_result_timeout() {
  let future = FutureProcess::new(Duration::from_millis(50));

  sleep(Duration::from_millis(100)).await;

  let result = future.result().await;
  assert!(matches!(result.unwrap_err(), FutureError::Timeout));
}

#[tokio::test]
async fn test_future_result_success() {
  let future = FutureProcess::new(Duration::from_secs(1));
  future.complete(MessageHandle::new("response".to_string())).await;

  let result = assert_future_success(&future).await;
  assert_eq!(result.as_any().downcast_ref::<String>().unwrap(), "response");
}
