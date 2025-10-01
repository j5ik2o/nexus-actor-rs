use crate::collections::element::Element;
use crate::collections::queue_sync::{SyncQueueBase, SyncQueueReader, SyncQueueWriter};
use crate::collections::{MpscBoundedChannelQueue, QueueError, QueueSize};

#[derive(Debug, Clone, PartialEq)]
struct TestElement(i32);

impl Element for TestElement {}

#[test]
fn test_new_queue() {
  let queue = MpscBoundedChannelQueue::<TestElement>::new(10);
  assert_eq!(queue.capacity(), QueueSize::Limited(10));
  assert_eq!(queue.len(), QueueSize::Limited(0));
}

#[test]
fn test_offer_and_poll() {
  let mut queue = MpscBoundedChannelQueue::<TestElement>::new(5);

  for i in 0..5 {
    assert!(queue.offer(TestElement(i)).is_ok());
  }

  assert_eq!(queue.len(), QueueSize::Limited(5));

  for i in 0..5 {
    let element = queue.poll().unwrap().unwrap();
    assert_eq!(element, TestElement(i));
  }

  assert_eq!(queue.len(), QueueSize::Limited(0));
  assert!(queue.poll().unwrap().is_none());
}

#[test]
fn test_offer_to_full_queue() {
  let mut queue = MpscBoundedChannelQueue::<TestElement>::new(2);

  assert!(queue.offer(TestElement(1)).is_ok());
  assert!(queue.offer(TestElement(2)).is_ok());

  match queue.offer(TestElement(3)) {
    Err(QueueError::OfferError(_)) => {}
    other => panic!("Expected OfferError, got {:?}", other),
  }

  assert_eq!(queue.len(), QueueSize::Limited(2));
}

#[test]
fn test_clean_up() {
  let mut queue = MpscBoundedChannelQueue::<TestElement>::new(5);

  for i in 0..3 {
    assert!(queue.offer(TestElement(i)).is_ok());
  }

  queue.clean_up();

  assert_eq!(queue.len(), QueueSize::Limited(0));
  match queue.poll() {
    Err(QueueError::PoolError) => {}
    other => panic!("Expected PoolError after clean_up, got {:?}", other),
  }

  match queue.offer(TestElement(4)) {
    Err(QueueError::OfferError(_)) => {}
    other => panic!("Expected OfferError after clean_up, got {:?}", other),
  }
}

#[tokio::test]
async fn test_concurrent_operations() {
  let queue = MpscBoundedChannelQueue::<TestElement>::new(100);
  let mut handles = vec![];

  for i in 0..10 {
    let mut q = queue.clone();
    handles.push(tokio::spawn(async move {
      for j in 0..10 {
        q.offer(TestElement(i * 10 + j)).unwrap();
      }
    }));
  }

  for _ in 0..5 {
    let mut q = queue.clone();
    handles.push(tokio::spawn(async move {
      let mut count = 0;
      while count < 20 {
        if q.poll().unwrap().is_some() {
          count += 1;
        }
      }
    }));
  }

  for handle in handles {
    handle.await.unwrap();
  }

  assert_eq!(queue.len(), QueueSize::Limited(0));
}
