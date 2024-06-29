use std::marker::PhantomData;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::util::element::Element;
use crate::util::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

pub const PRIORITY_LEVELS: usize = 8;
pub const DEFAULT_PRIORITY: i8 = (PRIORITY_LEVELS / 2) as i8;

pub trait PriorityMessage: Element {
  fn get_priority(&self) -> Option<i8>;
}

#[derive(Debug, Clone)]
pub struct PriorityQueue<E, Q> {
  priority_queues: Arc<Mutex<Vec<Q>>>,
  phantom_data: PhantomData<E>,
}

impl<E: PriorityMessage, Q: Clone + QueueReader<E> + QueueWriter<E>> PriorityQueue<E, Q> {
  pub fn new(queue_producer: impl Fn() -> Q + 'static) -> Self {
    let mut queues = Vec::with_capacity(PRIORITY_LEVELS);
    for _ in 0..PRIORITY_LEVELS {
      let queue = queue_producer();
      queues.push(queue);
    }
    Self {
      priority_queues: Arc::new(Mutex::new(queues)),
      phantom_data: PhantomData,
    }
  }
}

#[async_trait]
impl<E: PriorityMessage, Q: QueueReader<E> + QueueWriter<E>> QueueBase<E> for PriorityQueue<E, Q> {
  async fn len(&self) -> QueueSize {
    let queues_mg = self.priority_queues.lock().await;
    let mut len = 0;
    for queue in queues_mg.iter() {
      len += queue.len().await.to_usize();
    }
    QueueSize::Limited(len)
  }

  async fn capacity(&self) -> QueueSize {
    let queues_mg = self.priority_queues.lock().await;
    let mut capacity = QueueSize::Limited(0);
    for queue in queues_mg.iter() {
      capacity = capacity + queue.capacity().await;
    }
    capacity
  }
}

#[async_trait]
impl<E: PriorityMessage, Q: QueueReader<E> + QueueWriter<E>> QueueReader<E> for PriorityQueue<E, Q> {
  async fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    for p in (0..PRIORITY_LEVELS).rev() {
      let mut priority_queues_mg = self.priority_queues.lock().await;
      if let Ok(Some(item)) = priority_queues_mg[p].poll().await {
        return Ok(Some(item));
      }
    }
    Ok(None)
  }

  async fn clean_up(&mut self) {
    let mut mg = self.priority_queues.lock().await;
    for queue in mg.iter_mut() {
      queue.clean_up().await;
    }
  }
}

#[async_trait]
impl<E: PriorityMessage, Q: QueueReader<E> + QueueWriter<E>> QueueWriter<E> for PriorityQueue<E, Q> {
  async fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    let mut item_priority = DEFAULT_PRIORITY.clone();
    if let Some(priority) = element.get_priority() {
      item_priority = priority;
      if item_priority < 0 {
        item_priority = 0;
      }
      if item_priority >= PRIORITY_LEVELS as i8 - 1 {
        item_priority = PRIORITY_LEVELS as i8 - 1;
      }
    }
    let mut mg = self.priority_queues.lock().await;
    mg[item_priority as usize].offer(element).await
  }
}

#[cfg(test)]
mod tests {
  use std::any::Any;
  use std::fmt::Debug;

  use futures::StreamExt;

  use crate::actor::message::Message;
  use crate::util::queue::mpsc_unbounded_channel_queue::MpscUnboundedChannelQueue;

  use super::*;

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

  impl Message for TestPriorityMessage {
    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  impl TestMessageBase for TestPriorityMessage {
    fn get_message(&self) -> String {
      self.message.clone()
    }
  }

  #[derive(Debug, Clone)]
  struct TestMessage {
    message: String,
  }

  impl Element for TestMessage {}

  impl Message for TestMessage {
    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

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

  impl TestMessage {
    fn new(message: String) -> Self {
      Self { message }
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

  async fn new_priority_mspc_queue<M>() -> PriorityQueue<M, MpscUnboundedChannelQueue<M>>
  where
    M: TestMessageBase + Clone, {
    let queue = PriorityQueue::new(|| MpscUnboundedChannelQueue::new());
    assert_eq!(queue.len().await, QueueSize::Limited(0));
    assert_eq!(queue.capacity().await, QueueSize::Limited(0));
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

    for i in 0..2 {
      let msg = TestMessageBaseHandle::new(TestPriorityMessage::new("7 hello".to_string(), 7));
      q.offer(msg.clone()).await.unwrap();
    }

    for i in 0..2 {
      let msg = TestMessageBaseHandle::new(TestPriorityMessage::new("5 hello".to_string(), 5));
      q.offer(msg.clone()).await.unwrap();
    }

    for i in 0..2 {
      let msg = TestMessageBaseHandle::new(TestPriorityMessage::new("0 hello".to_string(), 0));
      q.offer(msg.clone()).await.unwrap();
    }

    for i in 0..2 {
      let msg = TestMessageBaseHandle::new(TestPriorityMessage::new("6 hello".to_string(), 6));
      q.offer(msg.clone()).await.unwrap();
    }

    for i in 0..2 {
      let msg = TestMessageBaseHandle::new(TestMessage::new("hello".to_string()));
      q.offer(msg.clone()).await.unwrap();
    }

    for i in 0..2 {
      let result = q.poll().await.unwrap();
      assert_eq!(result.unwrap().get_message(), "7 hello");
    }

    for i in 0..2 {
      let result = q.poll().await.unwrap();
      assert_eq!(result.unwrap().get_message(), "6 hello");
    }

    for i in 0..2 {
      let result = q.poll().await.unwrap();
      assert_eq!(result.unwrap().get_message(), "5 hello");
    }

    for i in 0..2 {
      let result = q.poll().await.unwrap();
      assert_eq!(result.unwrap().get_message(), "hello");
    }

    for i in 0..2 {
      let result = q.poll().await.unwrap();
      assert_eq!(result.unwrap().get_message(), "0 hello");
    }
  }
}
