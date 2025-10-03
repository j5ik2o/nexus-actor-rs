use std::time::Duration;
use tokio::time::sleep;

use crate::actor::dispatch::bounded::BoundedMailboxQueue;
use crate::actor::message::MessageHandle;
use nexus_utils_std_rs::collections::{QueueReader, QueueWriter, RingQueue};

fn setup_test_environment() -> BoundedMailboxQueue {
  BoundedMailboxQueue::new(RingQueue::new(3).with_dynamic(false), 3, false)
}

#[test]
fn test_bounded_queue_overflow() {
  let mut queue = setup_test_environment();
  assert!(queue.offer(MessageHandle::new("1".to_string())).is_ok());
  assert!(queue.offer(MessageHandle::new("2".to_string())).is_ok());
  assert!(queue.offer(MessageHandle::new("3".to_string())).is_err());
  let msg = queue.poll().unwrap().unwrap();
  assert_eq!(msg.to_typed::<String>().unwrap(), "1");
}

#[test]
fn test_bounded_queue_fifo_order() {
  let mut queue = setup_test_environment();

  for i in 1..=2 {
    assert!(queue.offer(MessageHandle::new(i)).is_ok());
  }

  for i in 1..=2 {
    let msg = queue.poll().unwrap().unwrap();
    assert_eq!(msg.to_typed::<i32>().unwrap(), i);
  }
}

#[test]
fn test_bounded_queue_poll_empty() {
  let mut queue = setup_test_environment();
  assert!(queue.poll().unwrap().is_none());
}

#[tokio::test]
async fn test_bounded_queue_concurrent_operations() {
  let queue = setup_test_environment();
  let queue_clone = queue.clone();

  let producer = async move {
    let mut q = queue;
    for i in 1..=4 {
      while q.offer(MessageHandle::new(i)).is_err() {
        sleep(Duration::from_millis(10)).await;
      }
    }
  };

  let consumer = async move {
    let mut q = queue_clone;
    let mut sum = 0;
    for _ in 1..=4 {
      if let Some(msg) = q.poll().unwrap() {
        sum += msg.to_typed::<i32>().unwrap();
      }
      sleep(Duration::from_millis(20)).await;
    }
    sum
  };

  let (_, sum) = tokio::join!(producer, consumer);
  assert_eq!(sum, 10); // 1 + 2 + 3 + 4
}
