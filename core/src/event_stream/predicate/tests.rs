use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::actor::core_types::message_types::Message;
use crate::actor::message::MessageHandle;
use crate::event_stream::predicate::Predicate;

#[tokio::test]
async fn test_predicate_thread_safety() {
  let counter = Arc::new(Mutex::new(0));
  let predicate = Predicate::new({
    let counter = counter.clone();
    move |_| {
      let counter = counter.clone();
      tokio::spawn(async move {
        let mut lock = counter.lock().await;
        *lock += 1;
      });
      true
    }
  });

  let handles: Vec<_> = (0..10)
    .map(|_| {
      let pred = predicate.clone();
      tokio::spawn(async move {
        pred.run(MessageHandle::new("test".to_string()))
      })
    })
    .collect();

  for handle in handles {
    assert!(handle.await.unwrap());
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