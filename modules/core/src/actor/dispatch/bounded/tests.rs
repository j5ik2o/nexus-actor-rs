use std::time::Duration;
use tokio::time::sleep;

use crate::actor::dispatch::bounded::BoundedMailboxQueue;
use crate::actor::message::MessageHandle;
use nexus_actor_utils_rs::collections::{QueueReader, QueueWriter, RingQueue};

async fn setup_test_environment() -> BoundedMailboxQueue {
  BoundedMailboxQueue::new(RingQueue::new(3).with_dynamic(false), 3, false)
}

#[tokio::test]
async fn test_bounded_queue_overflow() {
  let mut queue = setup_test_environment().await;
  assert!(queue.offer(MessageHandle::new("1".to_string())).await.is_ok());
  assert!(queue.offer(MessageHandle::new("2".to_string())).await.is_ok());
  assert!(queue.offer(MessageHandle::new("3".to_string())).await.is_err());
  let msg = queue.poll().await.unwrap().unwrap();
  assert_eq!(msg.to_typed::<String>().unwrap(), "1");
}

#[tokio::test]
async fn test_bounded_queue_fifo_order() {
  let mut queue = setup_test_environment().await;

  for i in 1..=2 {
    assert!(queue.offer(MessageHandle::new(i)).await.is_ok());
  }

  for i in 1..=2 {
    let msg = queue.poll().await.unwrap().unwrap();
    assert_eq!(msg.to_typed::<i32>().unwrap(), i);
  }
}

#[tokio::test]
async fn test_bounded_queue_poll_empty() {
  let mut queue = setup_test_environment().await;
  assert!(queue.poll().await.unwrap().is_none());
}

#[tokio::test]
async fn test_bounded_queue_concurrent_operations() {
  let queue = setup_test_environment().await;
  let queue_clone = queue.clone();

  let producer = tokio::spawn(async move {
    let mut q = queue;
    for i in 1..=4 {
      while q.offer(MessageHandle::new(i)).await.is_err() {
        sleep(Duration::from_millis(10)).await;
      }
    }
  });

  let consumer = tokio::spawn(async move {
    let mut q = queue_clone;
    let mut sum = 0;
    for _ in 1..=4 {
      if let Some(msg) = q.poll().await.unwrap() {
        sum += msg.to_typed::<i32>().unwrap();
      }
      sleep(Duration::from_millis(20)).await;
    }
    sum
  });

  producer.await.unwrap();
  let sum = consumer.await.unwrap();
  assert_eq!(sum, 10); // 1 + 2 + 3 + 4
}
