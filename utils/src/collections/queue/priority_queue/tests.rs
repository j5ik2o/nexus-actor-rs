use crate::collections::element::Element;
use crate::collections::{
  MpscUnboundedChannelQueue, PriorityMessage, PriorityQueue, QueueBase, QueueReader, QueueSize, QueueWriter,
};
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestPriorityMessage {
  message: String,
  priority: i8,
}

impl TestPriorityMessage {
  fn new(message: String, priority: i8) -> Self {
    Self { message, priority }
  }
}

impl Element for TestPriorityMessage {}

impl PriorityMessage for TestPriorityMessage {
  fn get_priority(&self) -> Option<i8> {
    Some(self.priority)
  }
}

impl TestMessageBase for TestPriorityMessage {
  fn get_message(&self) -> String {
    self.message.clone()
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestMessage {
  message: String,
}

impl TestMessage {
  fn new(message: String) -> Self {
    Self { message }
  }
}

impl Element for TestMessage {}

impl TestMessageBase for TestMessage {
  fn get_message(&self) -> String {
    self.message.clone()
  }
}

impl PriorityMessage for TestMessage {
  fn get_priority(&self) -> Option<i8> {
    None
  }
}

trait TestMessageBase: PriorityMessage {
  fn get_message(&self) -> String;
}

#[derive(Debug, Clone)]
struct TestMessageBaseHandle(Arc<dyn TestMessageBase>);

impl TestMessageBaseHandle {
  fn new(msg: impl TestMessageBase) -> Self {
    TestMessageBaseHandle(Arc::new(msg))
  }
}

impl PriorityMessage for TestMessageBaseHandle {
  fn get_priority(&self) -> Option<i8> {
    self.0.get_priority()
  }
}

impl Element for TestMessageBaseHandle {}

impl TestMessageBase for TestMessageBaseHandle {
  fn get_message(&self) -> String {
    self.0.get_message()
  }
}

async fn new_priority_ring_queue<M>() -> PriorityQueue<M, MpscUnboundedChannelQueue<M>>
where
  M: TestMessageBase + Clone, {
  let queue = PriorityQueue::new(|| MpscUnboundedChannelQueue::new());
  assert_eq!(queue.len().await, QueueSize::Limited(0));
  assert_eq!(queue.capacity().await, QueueSize::Limitless);
  queue
}

#[tokio::test]
async fn test_push_pop_ring() {
  let mut q = new_priority_ring_queue().await;
  let msg = TestPriorityMessage::new("hello".to_string(), 0);
  q.offer(msg.clone()).await.unwrap();
  let result = q.poll().await.unwrap();
  assert_eq!(result, Some(msg));
}

#[tokio::test]
async fn test_push_pop_ring_2() {
  let mut q: PriorityQueue<TestMessageBaseHandle, MpscUnboundedChannelQueue<TestMessageBaseHandle>> =
    new_priority_ring_queue().await;

  for _ in 0..2 {
    let msg = TestMessageBaseHandle::new(TestPriorityMessage::new("7 hello".to_string(), 7));
    q.offer(msg.clone()).await.unwrap();
  }

  for _ in 0..2 {
    let msg = TestMessageBaseHandle::new(TestPriorityMessage::new("5 hello".to_string(), 5));
    q.offer(msg.clone()).await.unwrap();
  }

  for _ in 0..2 {
    let msg = TestMessageBaseHandle::new(TestPriorityMessage::new("0 hello".to_string(), 0));
    q.offer(msg.clone()).await.unwrap();
  }

  for _ in 0..2 {
    let msg = TestMessageBaseHandle::new(TestPriorityMessage::new("6 hello".to_string(), 6));
    q.offer(msg.clone()).await.unwrap();
  }

  for _ in 0..2 {
    let msg = TestMessageBaseHandle::new(TestMessage::new("hello".to_string()));
    q.offer(msg.clone()).await.unwrap();
  }

  for _ in 0..2 {
    let result = q.poll().await.unwrap();
    assert_eq!(result.unwrap().get_message(), "7 hello");
  }

  for _ in 0..2 {
    let result = q.poll().await.unwrap();
    assert_eq!(result.unwrap().get_message(), "6 hello");
  }

  for _ in 0..2 {
    let result = q.poll().await.unwrap();
    assert_eq!(result.unwrap().get_message(), "5 hello");
  }

  for _ in 0..2 {
    let result = q.poll().await.unwrap();
    assert_eq!(result.unwrap().get_message(), "hello");
  }

  for _ in 0..2 {
    let result = q.poll().await.unwrap();
    assert_eq!(result.unwrap().get_message(), "0 hello");
  }
}
