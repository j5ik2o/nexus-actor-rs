use crate::collections::{QueueBase, QueueReader, QueueWriter};
use crate::collections::{QueueError, RingQueue};

#[test]
fn test_push_pop() {
  let mut queue = RingQueue::new(5);
  assert!(queue.offer(1).is_ok());
  assert!(queue.offer(2).is_ok());
  assert_eq!(queue.poll().unwrap(), Some(1));
  assert_eq!(queue.poll().unwrap(), Some(2));
  assert_eq!(queue.poll().unwrap(), None);
}

#[test]
fn test_full_queue_fixed_size() {
  let mut queue = RingQueue::new(4).with_dynamic(false);
  assert!(queue.offer(1).is_ok());
  assert!(queue.offer(2).is_ok());
  assert!(queue.offer(3).is_ok());
  assert!(matches!(queue.offer(4), Err(QueueError::OfferError(4))));
}

#[test]
fn test_full_queue_dynamic_size() {
  let mut queue = RingQueue::new(4).with_dynamic(true);
  for i in 1..=5 {
    assert!(queue.offer(i).is_ok());
  }
  assert_eq!(queue.capacity().to_usize(), 9);

  for i in 6..=9 {
    assert!(queue.offer(i).is_ok());
  }
  assert_eq!(queue.capacity().to_usize(), 19);
}

#[test]
fn test_len_and_is_empty() {
  let mut queue = RingQueue::new(5);
  assert_eq!(queue.len().to_usize(), 0);

  queue.offer(1).unwrap();
  assert_eq!(queue.len().to_usize(), 1);

  queue.offer(2).unwrap();
  assert_eq!(queue.len().to_usize(), 2);

  queue.poll().unwrap();
  assert_eq!(queue.len().to_usize(), 1);

  queue.poll().unwrap();
  assert_eq!(queue.len().to_usize(), 0);
}

#[test]
fn test_wrap_around() {
  let mut queue = RingQueue::new(4);
  for i in 1..=4 {
    assert!(queue.offer(i).is_ok());
  }
  assert_eq!(queue.poll().unwrap(), Some(1));
  assert!(queue.offer(5).is_ok());
  assert_eq!(queue.poll().unwrap(), Some(2));
  assert_eq!(queue.poll().unwrap(), Some(3));
  assert_eq!(queue.poll().unwrap(), Some(4));
  assert_eq!(queue.poll().unwrap(), Some(5));
  assert_eq!(queue.poll().unwrap(), None);
}

#[test]
fn test_clean_up() {
  let mut queue = RingQueue::new(5);
  queue.offer(1).unwrap();
  queue.offer(2).unwrap();
  queue.offer(3).unwrap();
  assert_eq!(queue.len().to_usize(), 3);

  queue.clean_up();
  assert_eq!(queue.len().to_usize(), 0);
  assert_eq!(queue.poll().unwrap(), None);
}
