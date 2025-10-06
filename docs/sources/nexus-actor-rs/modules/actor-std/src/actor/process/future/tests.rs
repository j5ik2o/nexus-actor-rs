use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::ExtendedPid;
use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use crate::actor::process::future::{ActorFutureError, ActorFutureProcess};
use crate::actor::process::{Process, ProcessHandle};
use async_trait::async_trait;
use nexus_actor_core_rs::runtime::CoreSpawner as _;
use nexus_actor_core_rs::runtime::CoreTaskFuture;
use nexus_utils_std_rs::concurrent::AsyncBarrier;
use nexus_utils_std_rs::runtime::TokioCoreSpawner;
use tokio::sync::Notify;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone)]
struct MockProcess {
  name: String,
  received: Arc<AtomicBool>,
  notify: Arc<Notify>,
  pid: Option<ExtendedPid>,
}

impl MockProcess {
  async fn new(actor_system: ActorSystem, name: &str) -> Self {
    let mut process = MockProcess {
      name: name.to_string(),
      received: Arc::new(AtomicBool::new(false)),
      notify: Arc::new(Notify::new()),
      pid: None,
    };
    let id = actor_system.get_process_registry().await.next_id();
    let (pid, ok) = actor_system
      .get_process_registry()
      .await
      .add_process(ProcessHandle::new(process.clone()), &format!("mock_{}", id))
      .await;
    if !ok {
      panic!("failed to register mock process");
    }
    process.pid = Some(pid);
    process
  }

  pub fn get_pid(&self) -> ExtendedPid {
    self.pid.clone().unwrap()
  }
}

#[async_trait]
impl Process for MockProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, _: MessageHandle) {
    tracing::debug!("MockProcess {} received message", self.name); // デバッグログ
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
  let system = ActorSystem::new().await.unwrap();
  let a1 = Arc::new(MockProcess::new(system.clone(), "a1").await);
  let a2 = Arc::new(MockProcess::new(system.clone(), "a2").await);
  let a3 = Arc::new(MockProcess::new(system.clone(), "a3").await);

  let barrier = AsyncBarrier::new(4);

  let future_process = ActorFutureProcess::new(system, Duration::from_secs(1)).await;

  future_process.pipe_to(a1.get_pid()).await;
  future_process.pipe_to(a2.get_pid()).await;
  future_process.pipe_to(a3.get_pid()).await;

  // モックプロセスにバリアを設定
  for process in [a1.clone(), a2.clone(), a3.clone()] {
    let barrier = barrier.clone();
    let received = process.notify.clone();
    let task: CoreTaskFuture = Box::pin(async move {
      received.notified().await;
      barrier.wait().await;
    });
    TokioCoreSpawner::current().spawn(task).expect("spawn watcher").detach();
  }

  future_process
    .send_user_message(None, MessageHandle::new("hello".to_string()))
    .await;

  // タイムアウト付きで待機
  let timeout_result = tokio::time::timeout(Duration::from_secs(5), barrier.wait()).await;
  assert!(
    timeout_result.is_ok(),
    "Test timed out waiting for all processes to receive the message"
  );

  for process in [a1.clone(), a2.clone(), a3.clone()] {
    assert!(
      process.received.load(Ordering::SeqCst),
      "{} did not receive message",
      process.name
    );
  }

  // FutureProcess の状態を確認
  let is_empty = future_process.is_empty().await;
  assert!(!is_empty, "future should not be empty after completion");

  // FutureProcess の結果を確認
  let result = future_process.result().await;
  assert!(result.is_ok(), "Expected Ok result, got {:?}", result);
  let message = result.unwrap();
  assert_eq!(message.as_any().downcast_ref::<String>().unwrap(), "hello");
}

#[tokio::test]
async fn test_future_pipe_to_timeout_sends_error() {
  let system = ActorSystem::new().await.unwrap();
  let a1 = Arc::new(MockProcess::new(system.clone(), "a1").await);
  let a2 = Arc::new(MockProcess::new(system.clone(), "a2").await);
  let a3 = Arc::new(MockProcess::new(system.clone(), "a3").await);

  let barrier = AsyncBarrier::new(4);

  let future_process = ActorFutureProcess::new(system, Duration::from_millis(100)).await;

  future_process.pipe_to(a1.get_pid()).await;
  future_process.pipe_to(a2.get_pid()).await;
  future_process.pipe_to(a3.get_pid()).await;

  for process in [a1.clone(), a2.clone(), a3.clone()] {
    let barrier = barrier.clone();
    let received = process.notify.clone();
    let task: CoreTaskFuture = Box::pin(async move {
      received.notified().await;
      barrier.wait().await;
    });
    TokioCoreSpawner::current().spawn(task).expect("spawn watcher").detach();
  }

  let err = future_process.result().await;
  assert!(err.is_err());
  assert!(matches!(err.unwrap_err(), ActorFutureError::TimeoutError));

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
  let system = ActorSystem::new().await.unwrap();
  let future_process = ActorFutureProcess::new(system, Duration::from_millis(200)).await;
  let barrier = AsyncBarrier::new(2);

  let future = future_process.clone();
  let barrier_clone = barrier.clone();
  let task: CoreTaskFuture = Box::pin(async move {
    match tokio::time::timeout(
      Duration::from_millis(100),
      future.complete(MessageHandle::new("response".to_string())),
    )
    .await
    {
      Ok(_) => {}
      Err(_) => panic!("timeout"),
    }

    barrier_clone.wait().await;
  });
  TokioCoreSpawner::current()
    .spawn(task)
    .expect("spawn timeout completer")
    .detach();

  barrier.wait().await;

  let result = future_process.result().await;
  assert!(result.is_ok(), "Expected Ok, got {:?}", result);
}

async fn assert_future_success(future_process: &ActorFutureProcess) -> MessageHandle {
  match future_process.result().await {
    Ok(res) => res,
    Err(e) => panic!("Future failed: {:?}", e),
  }
}

#[tokio::test]
async fn test_future_result_dead_letter_response() {
  let system = ActorSystem::new().await.unwrap();
  let future_process = ActorFutureProcess::new(system, Duration::from_secs(1)).await;
  future_process.fail(ActorFutureError::DeadLetterError).await;

  let result = future_process.result().await;
  assert!(matches!(result.unwrap_err(), ActorFutureError::DeadLetterError));
}

#[tokio::test]
async fn test_future_result_timeout() {
  let system = ActorSystem::new().await.unwrap();
  let future_process = ActorFutureProcess::new(system, Duration::from_millis(50)).await;

  sleep(Duration::from_millis(100)).await;

  let result = future_process.result().await;
  assert!(matches!(result.unwrap_err(), ActorFutureError::TimeoutError));
}

#[tokio::test]
async fn test_future_result_success() {
  let system = ActorSystem::new().await.unwrap();
  let future_process = ActorFutureProcess::new(system, Duration::from_secs(1)).await;
  future_process
    .complete(MessageHandle::new("response".to_string()))
    .await;

  let result = assert_future_success(&future_process).await;
  assert_eq!(result.as_any().downcast_ref::<String>().unwrap(), "response");
}
