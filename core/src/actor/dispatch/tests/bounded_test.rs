#[cfg(test)]
mod tests {
  use tokio::time::sleep;

  use crate::actor::dispatch::bounded::BoundedQueue;
  use crate::actor::message::MessageHandle;

  async fn setup_test_environment() -> BoundedQueue {
    BoundedQueue::new(2)
  }

  #[tokio::test]
  async fn test_bounded_queue_overflow() {
    let queue = setup_test_environment().await;
    assert!(queue.offer(MessageHandle::new("1")).await);
    assert!(queue.offer(MessageHandle::new("2")).await);
    assert!(!queue.offer(MessageHandle::new("3")).await);
    assert_eq!(queue.poll().await.unwrap().to_typed::<&str>().unwrap(), "1");
  }

  #[tokio::test]
  async fn test_bounded_queue_fifo_order() {
    let queue = setup_test_environment().await;
    
    for i in 1..=2 {
      assert!(queue.offer(MessageHandle::new(i)).await);
    }

    for i in 1..=2 {
      let msg = queue.poll().await.unwrap();
      assert_eq!(msg.to_typed::<i32>().unwrap(), i);
    }
  }

  #[tokio::test]
  async fn test_bounded_queue_poll_empty() {
    let queue = setup_test_environment().await;
    assert!(queue.poll().await.is_none());
  }

  #[tokio::test]
  async fn test_bounded_queue_concurrent_operations() {
    let queue = setup_test_environment().await;
    let queue_clone = queue.clone();

    let producer = tokio::spawn(async move {
      for i in 1..=4 {
        while !queue.offer(MessageHandle::new(i)).await {
          sleep(Duration::from_millis(10)).await;
        }
      }
    });

    let consumer = tokio::spawn(async move {
      let mut sum = 0;
      for _ in 1..=4 {
        if let Some(msg) = queue_clone.poll().await {
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
}
