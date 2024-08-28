#[cfg(test)]
mod tests {
  use crate::util::queue::ring_queue::RingQueue;
  use crate::util::queue::{QueueBase, QueueError, QueueReader, QueueWriter};

  #[tokio::test]
  async fn test_push_pop() {
    let mut queue = RingQueue::new(5);
    assert!(queue.offer(1).await.is_ok());
    assert!(queue.offer(2).await.is_ok());
    assert_eq!(queue.poll().await.unwrap(), Some(1));
    assert_eq!(queue.poll().await.unwrap(), Some(2));
    assert_eq!(queue.poll().await.unwrap(), None);
  }

  #[tokio::test]
  async fn test_full_queue_fixed_size() {
    let mut queue = RingQueue::new(4).with_dynamic(false);
    assert!(queue.offer(1).await.is_ok());
    assert!(queue.offer(2).await.is_ok());
    assert!(queue.offer(3).await.is_ok());
    assert!(matches!(queue.offer(4).await, Err(QueueError::OfferError(4))));
  }

  #[tokio::test]
  async fn test_full_queue_dynamic_size() {
    let mut queue = RingQueue::new(4).with_dynamic(true);
    assert!(queue.offer(1).await.is_ok());
    assert!(queue.offer(2).await.is_ok());
    assert!(queue.offer(3).await.is_ok());
    assert!(queue.offer(4).await.is_ok());
    assert!(queue.offer(5).await.is_ok()); // This should trigger resize
    assert_eq!(queue.capacity().await.to_usize(), 9);

    // Add more elements to test the new capacity
    assert!(queue.offer(6).await.is_ok());
    assert!(queue.offer(7).await.is_ok());
    assert!(queue.offer(8).await.is_ok());
    assert!(queue.offer(9).await.is_ok()); // This should trigger another resize
    assert_eq!(queue.capacity().await.to_usize(), 19);
  }

  #[tokio::test]
  async fn test_len_and_is_empty() {
    let mut queue = RingQueue::new(5);
    assert_eq!(queue.len().await.to_usize(), 0);

    queue.offer(1).await.unwrap();
    assert_eq!(queue.len().await.to_usize(), 1);

    queue.offer(2).await.unwrap();
    assert_eq!(queue.len().await.to_usize(), 2);

    queue.poll().await.unwrap();
    assert_eq!(queue.len().await.to_usize(), 1);

    queue.poll().await.unwrap();
    assert_eq!(queue.len().await.to_usize(), 0);
  }

  #[tokio::test]
  async fn test_wrap_around() {
    let mut queue = RingQueue::new(4);
    assert!(queue.offer(1).await.is_ok());
    assert!(queue.offer(2).await.is_ok());
    assert!(queue.offer(3).await.is_ok());
    assert!(queue.offer(4).await.is_ok());
    assert_eq!(queue.poll().await.unwrap(), Some(1));
    assert!(queue.offer(5).await.is_ok());
    assert_eq!(queue.poll().await.unwrap(), Some(2));
    assert_eq!(queue.poll().await.unwrap(), Some(3));
    assert_eq!(queue.poll().await.unwrap(), Some(4));
    assert_eq!(queue.poll().await.unwrap(), Some(5));
    assert_eq!(queue.poll().await.unwrap(), None);
  }

  #[tokio::test]
  async fn test_clean_up() {
    let mut queue = RingQueue::new(5);
    queue.offer(1).await.unwrap();
    queue.offer(2).await.unwrap();
    queue.offer(3).await.unwrap();
    assert_eq!(queue.len().await.to_usize(), 3);

    queue.clean_up().await;
    assert_eq!(queue.len().await.to_usize(), 0);
    assert_eq!(queue.poll().await.unwrap(), None);
  }
}
