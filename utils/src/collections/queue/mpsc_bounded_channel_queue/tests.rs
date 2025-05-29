#[cfg(test)]
mod tests {
  use crate::collections::element::Element;
  use crate::collections::queue::mpsc_bounded_channel_queue::MpscBoundedChannelQueue;
  use crate::collections::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

  #[derive(Debug, Clone, PartialEq)]
  struct TestElement(i32);

  impl Element for TestElement {}

  #[tokio::test]
  async fn test_new_queue() {
    let queue = MpscBoundedChannelQueue::<TestElement>::new(10);
    assert_eq!(queue.capacity().await, QueueSize::Limited(10));
    assert_eq!(queue.len().await, QueueSize::Limited(0));
  }

  #[tokio::test]
  async fn test_offer_and_poll() {
    let mut queue = MpscBoundedChannelQueue::<TestElement>::new(5);

    // Offer elements
    for i in 0..5 {
      assert!(queue.offer(TestElement(i)).await.is_ok());
    }

    // Check queue size
    assert_eq!(queue.len().await, QueueSize::Limited(5));

    // Poll elements
    for i in 0..5 {
      let element = queue.poll().await.unwrap().unwrap();
      assert_eq!(element, TestElement(i));
    }

    // Queue should be empty now
    assert_eq!(queue.len().await, QueueSize::Limited(0));
    assert!(queue.poll().await.unwrap().is_none());
  }

  #[tokio::test]
  async fn test_offer_to_full_queue() {
    let mut queue = MpscBoundedChannelQueue::<TestElement>::new(2);

    assert!(queue.offer(TestElement(1)).await.is_ok());
    assert!(queue.offer(TestElement(2)).await.is_ok());

    // This should fail as the queue is full
    match queue.offer(TestElement(3)).await {
      Err(QueueError::OfferError(_)) => (),
      other => panic!("Expected OfferError, got {:?}", other),
    }

    // Ensure the queue size is still 2
    assert_eq!(queue.len().await, QueueSize::Limited(2));
  }

  #[tokio::test]
  async fn test_clean_up() {
    let mut queue = MpscBoundedChannelQueue::<TestElement>::new(5);

    for i in 0..3 {
      assert!(queue.offer(TestElement(i)).await.is_ok());
    }

    queue.clean_up().await;

    assert_eq!(queue.len().await, QueueSize::Limited(0));
    match queue.poll().await {
      Err(QueueError::PoolError) => (),
      other => panic!("Expected PoolError after clean_up, got {:?}", other),
    }

    // Additional test to ensure offer also fails after clean_up
    match queue.offer(TestElement(4)).await {
      Err(QueueError::OfferError(_)) => (),
      other => panic!("Expected OfferError after clean_up, got {:?}", other),
    }
  }

  #[tokio::test]
  async fn test_concurrent_operations() {
    let queue = MpscBoundedChannelQueue::<TestElement>::new(100);
    let mut handles = vec![];

    // Spawn 10 tasks to offer elements
    for i in 0..10 {
      let mut q = queue.clone();
      handles.push(tokio::spawn(async move {
        for j in 0..10 {
          q.offer(TestElement(i * 10 + j)).await.unwrap();
        }
      }));
    }

    // Spawn 5 tasks to poll elements
    for _ in 0..5 {
      let mut q = queue.clone();
      handles.push(tokio::spawn(async move {
        let mut count = 0;
        while count < 20 {
          if let Some(_) = q.poll().await.unwrap() {
            count += 1;
          }
        }
      }));
    }

    // Wait for all tasks to complete
    for handle in handles {
      handle.await.unwrap();
    }

    // Check final queue state
    assert_eq!(queue.len().await, QueueSize::Limited(0));
  }
}
