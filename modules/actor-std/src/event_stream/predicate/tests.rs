use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use nexus_actor_core_rs::runtime::CoreSpawner as _;
use nexus_actor_core_rs::runtime::{CoreJoinHandle, CoreTaskFuture};
use nexus_utils_std_rs::runtime::TokioCoreSpawner;
use tokio::sync::{oneshot, Mutex};

use crate::actor::core_types::message_types::Message;
use crate::actor::message::MessageHandle;
use crate::event_stream::predicate::Predicate;

fn spawn_bool_task<Fut>(future: Fut) -> (Arc<dyn CoreJoinHandle>, oneshot::Receiver<bool>)
where
  Fut: std::future::Future<Output = bool> + Send + 'static, {
  let (tx, rx) = oneshot::channel();
  let task: CoreTaskFuture = Box::pin(async move {
    let result = future.await;
    let _ = tx.send(result);
  });
  let handle = TokioCoreSpawner::current().spawn(task).expect("spawn predicate task");
  (handle, rx)
}

#[tokio::test]
async fn test_predicate_thread_safety() {
  let counter = Arc::new(Mutex::new(0));
  let predicate = Predicate::new({
    let counter = counter.clone();
    move |_| {
      let counter = counter.clone();
      let task: CoreTaskFuture = Box::pin(async move {
        let mut lock = counter.lock().await;
        *lock += 1;
      });
      TokioCoreSpawner::current()
        .spawn(task)
        .expect("spawn counter increment")
        .detach();
      true
    }
  });

  let handles: Vec<_> = (0..10)
    .map(|_| {
      let pred = predicate.clone();
      spawn_bool_task(async move { pred.run(MessageHandle::new("test".to_string())) })
    })
    .collect();

  for (handle, rx) in handles {
    let result = rx.await.unwrap_or(false);
    handle.clone().join().await;
    assert!(result);
  }

  tokio::time::sleep(Duration::from_millis(100)).await;
  assert_eq!(*counter.lock().await, 10);
}

#[tokio::test]
async fn test_predicate_hash_consistency() {
  let pred1 = Predicate::new(|_| true);
  let pred2 = pred1.clone();

  let mut hasher1 = DefaultHasher::new();
  let mut hasher2 = DefaultHasher::new();
  pred1.hash(&mut hasher1);
  pred2.hash(&mut hasher2);

  assert_eq!(hasher1.finish(), hasher2.finish());
}

#[tokio::test]
async fn test_predicate_equality() {
  let pred1 = Predicate::new(|_| true);
  let pred2 = pred1.clone();
  let pred3 = Predicate::new(|_| true);

  assert_eq!(pred1, pred2);
  assert_ne!(pred1, pred3);
}

#[tokio::test]
async fn test_predicate_message_type_filtering() {
  let predicate = Predicate::new(|msg| msg.as_any().type_id() == std::any::TypeId::of::<String>());

  assert!(predicate.run(MessageHandle::new(String::from("test"))));
  assert!(!predicate.run(MessageHandle::new(42)));
}
