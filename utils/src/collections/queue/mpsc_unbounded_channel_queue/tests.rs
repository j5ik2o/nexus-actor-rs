#[cfg(test)]
mod tests {
  use crate::collections::element::Element;
  use crate::collections::{MpscUnboundedChannelQueue, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

  #[derive(Debug, Clone, PartialEq)]
  struct TestElement(i32);

  impl Element for TestElement {}

  #[tokio::test]
  async fn test_new_queue() {
    let queue = MpscUnboundedChannelQueue::<TestElement>::new();
    assert_eq!(queue.capacity().await, QueueSize::Limitless);
    assert_eq!(queue.len().await, QueueSize::Limited(0));
  }

  #[tokio::test]
  async fn test_offer_and_poll() {
    let mut queue = MpscUnboundedChannelQueue::<TestElement>::new();

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
  async fn test_clean_up() {
    let mut queue = MpscUnboundedChannelQueue::<TestElement>::new();

    for i in 0..3 {
      assert!(queue.offer(TestElement(i)).await.is_ok());
    }

    queue.clean_up().await;

    assert_eq!(queue.len().await, QueueSize::Limited(0));

    // Poll should return PoolError
    match queue.poll().await {
      Err(QueueError::PoolError) => (),
      other => panic!("Expected PoolError after clean_up, got {:?}", other),
    }

    // Offer should return OfferError
    match queue.offer(TestElement(4)).await {
      Err(QueueError::OfferError(_)) => (),
      other => panic!("Expected OfferError after clean_up, got {:?}", other),
    }

    // Another poll should still return PoolError
    match queue.poll().await {
      Err(QueueError::PoolError) => (),
      other => panic!("Expected PoolError after clean_up, got {:?}", other),
    }
  }

  #[tokio::test]
  async fn test_concurrent_operations() {
    let queue = MpscUnboundedChannelQueue::<TestElement>::new();
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

  #[tokio::test]
  async fn test_offer_to_large_queue() {
    let mut queue = MpscUnboundedChannelQueue::<TestElement>::new();

    // Offer a large number of elements
    for i in 0..10000 {
      assert!(queue.offer(TestElement(i)).await.is_ok());
    }

    assert_eq!(queue.len().await, QueueSize::Limited(10000));

    // Poll all elements
    for i in 0..10000 {
      let element = queue.poll().await.unwrap().unwrap();
      assert_eq!(element, TestElement(i));
    }

    assert_eq!(queue.len().await, QueueSize::Limited(0));
  }
}
