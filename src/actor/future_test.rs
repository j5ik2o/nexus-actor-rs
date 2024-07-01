use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use tokio::sync::{Mutex, Notify};
use tokio::time::{Duration, sleep};

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor_system::ActorSystem;
use crate::actor::future::{FutureError, FutureProcess};
use crate::actor::message::{Message, MessageHandle};
use crate::actor::process::Process;

#[derive(Clone)]
struct AsyncBarrier {
  notify: Arc<Notify>,
  count: Arc<Mutex<usize>>,
}

impl AsyncBarrier {
  fn new(count: usize) -> Self {
    AsyncBarrier {
      notify: Arc::new(Notify::new()),
      count: Arc::new(Mutex::new(count)),
    }
  }

  async fn wait(&self) {
    let mut count = self.count.lock().await;
    *count -= 1;
    if *count == 0 {
      self.notify.notify_waiters();
    } else {
      drop(count);
      self.notify.notified().await;
    }
  }
}

#[derive(Debug)]
struct MockProcess {
  name: String,
  received: Arc<AtomicBool>,
  notify: Arc<Notify>,
}

#[async_trait]
impl Process for MockProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, message: MessageHandle) {
    println!("MockProcess {} received message", self.name); // デバッグログ
    self.received.store(true, Ordering::SeqCst);
    self.notify.notify_one();
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
  let system = ActorSystem::new(&[]).await;
  let a1 = Arc::new(MockProcess {
    name: "a1".to_string(),
    received: Arc::new(AtomicBool::new(false)),
    notify: Arc::new(Notify::new()),
  });
  let a2 = Arc::new(MockProcess {
    name: "a2".to_string(),
    received: Arc::new(AtomicBool::new(false)),
    notify: Arc::new(Notify::new()),
  });
  let a3 = Arc::new(MockProcess {
    name: "a3".to_string(),
    received: Arc::new(AtomicBool::new(false)),
    notify: Arc::new(Notify::new()),
  });

  let barrier = AsyncBarrier::new(4);

  let future_process = FutureProcess::new(system, Duration::from_secs(1)).await;

  future_process.pipe_to(a1.clone()).await;
  future_process.pipe_to(a2.clone()).await;
  future_process.pipe_to(a3.clone()).await;

  // モックプロセスにバリアを設定
  for process in [a1.clone(), a2.clone(), a3.clone()] {
    let barrier = barrier.clone();
    let received = process.notify.clone();
    tokio::spawn(async move {
      received.notified().await;
      barrier.wait().await;
    });
  }

  future_process
    .send_user_message(None, MessageHandle::new("hello".to_string()))
    .await;

  barrier.wait().await;

  for process in [a1.clone(), a2.clone(), a3.clone()] {
    assert!(
      process.received.load(Ordering::SeqCst),
      "{} did not receive message",
      process.name
    );
  }

  assert!(
    !future_process.is_empty().await,
    "future should not be empty after completion"
  );
}

#[tokio::test]
async fn test_future_pipe_to_timeout_sends_error() {
  let system = ActorSystem::new(&[]).await;
  let a1 = Arc::new(MockProcess {
    name: "a1".to_string(),
    received: Arc::new(AtomicBool::new(false)),
    notify: Arc::new(Notify::new()),
  });
  let a2 = Arc::new(MockProcess {
    name: "a2".to_string(),
    received: Arc::new(AtomicBool::new(false)),
    notify: Arc::new(Notify::new()),
  });
  let a3 = Arc::new(MockProcess {
    name: "a3".to_string(),
    received: Arc::new(AtomicBool::new(false)),
    notify: Arc::new(Notify::new()),
  });

  let barrier = AsyncBarrier::new(4);

  let future_process = FutureProcess::new(system, Duration::from_millis(100)).await;

  future_process.pipe_to(a1.clone()).await;
  future_process.pipe_to(a2.clone()).await;
  future_process.pipe_to(a3.clone()).await;

  for process in [a1.clone(), a2.clone(), a3.clone()] {
    let barrier = barrier.clone();
    let received = process.notify.clone();
    tokio::spawn(async move {
      received.notified().await;
      barrier.wait().await;
    });
  }

  let err = future_process.result().await;
  assert!(err.is_err());
  assert!(matches!(err.unwrap_err(), FutureError::Timeout));

  barrier.wait().await;

  for process in [a1.clone(), a2.clone(), a3.clone()] {
    assert!(
      process.received.load(Ordering::SeqCst),
      "{} did not receive message",
      process.name
    );
  }

  assert!(
    !future_process.is_empty().await,
    "future should not be empty after timeout"
  );
}

#[tokio::test]
async fn test_new_future_timeout_no_race() {
  let system = ActorSystem::new(&[]).await;
  let future_process = FutureProcess::new(system, Duration::from_millis(200)).await;
  let barrier = AsyncBarrier::new(2);

  tokio::spawn({
    let future = future_process.clone();
    let barrier = barrier.clone();
    async move {
      sleep(Duration::from_millis(100)).await;
      future.complete(MessageHandle::new("response".to_string())).await;
      barrier.wait().await;
    }
  });

  barrier.wait().await;

  let result = future_process.result().await;
  assert!(result.is_ok(), "Expected Ok, got {:?}", result);
}

async fn assert_future_success(future_process: &FutureProcess) -> MessageHandle {
  match future_process.result().await {
    Ok(res) => res,
    Err(e) => panic!("Future failed: {:?}", e),
  }
}

#[tokio::test]
async fn test_future_result_dead_letter_response() {
  let system = ActorSystem::new(&[]).await;
  let future_process = FutureProcess::new(system, Duration::from_secs(1)).await;
  future_process.fail(FutureError::DeadLetter).await;

  let result = future_process.result().await;
  assert!(matches!(result.unwrap_err(), FutureError::DeadLetter));
}

#[tokio::test]
async fn test_future_result_timeout() {
  let system = ActorSystem::new(&[]).await;
  let future_process = FutureProcess::new(system, Duration::from_millis(50)).await;

  sleep(Duration::from_millis(100)).await;

  let result = future_process.result().await;
  assert!(matches!(result.unwrap_err(), FutureError::Timeout));
}

#[tokio::test]
async fn test_future_result_success() {
  let system = ActorSystem::new(&[]).await;
  let future_process = FutureProcess::new(system, Duration::from_secs(1)).await;
  future_process
    .complete(MessageHandle::new("response".to_string()))
    .await;

  let result = assert_future_success(&future_process).await;
  assert_eq!(result.as_any().downcast_ref::<String>().unwrap(), "response");
}
