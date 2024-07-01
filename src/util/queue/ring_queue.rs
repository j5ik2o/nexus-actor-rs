use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::mem;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::util::element::Element;
use crate::util::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

#[derive(Debug, Clone)]
pub struct RingQueue<E> {
  buffer: Arc<Mutex<Vec<Option<E>>>>,
  head: Arc<AtomicUsize>,
  tail: Arc<AtomicUsize>,
  capacity: Arc<AtomicUsize>,
  dynamic: Arc<AtomicBool>,
}

impl<E: Element> RingQueue<E> {
  pub fn new(capacity: usize) -> Self {
    assert!(capacity > 0, "Capacity must be greater than zero");
    let mut buffer = Vec::with_capacity(capacity);
    buffer.resize_with(capacity, || None);
    Self {
      buffer: Arc::new(Mutex::new(buffer)),
      head: Arc::new(AtomicUsize::new(0)),
      tail: Arc::new(AtomicUsize::new(0)),
      capacity: Arc::new(AtomicUsize::new(capacity)),
      dynamic: Arc::new(AtomicBool::new(true)),
    }
  }

  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.dynamic.store(dynamic, Ordering::Relaxed);
    self
  }

  fn is_full(&self) -> bool {
    let head = self.head.load(Ordering::Relaxed);
    let next_tail = (self.tail.load(Ordering::Relaxed) + 1) % self.capacity.load(Ordering::Relaxed);
    next_tail == head
  }

  async fn resize(&self) {
    let mut buffer = self.buffer.lock().await;
    let old_capacity = buffer.len();
    let new_capacity = old_capacity * 2 + 1; // +1 to maintain the extra space
    let mut new_buffer = Vec::with_capacity(new_capacity);
    new_buffer.resize_with(new_capacity, || None);

    let head = self.head.load(Ordering::Relaxed);
    let tail = self.tail.load(Ordering::Relaxed);

    // Move elements from the old buffer to the new buffer
    let mut count = 0;
    let mut i = head;
    while i != tail {
      new_buffer[count] = mem::replace(&mut buffer[i], None);
      i = (i + 1) % old_capacity;
      count += 1;
    }

    // Update the buffer and capacity
    *buffer = new_buffer;
    self.capacity.store(new_capacity, Ordering::Relaxed);
    self.head.store(0, Ordering::Relaxed);
    self.tail.store(count, Ordering::Relaxed);
  }
}

#[async_trait]
impl<E: Element> QueueBase<E> for RingQueue<E> {
  async fn len(&self) -> QueueSize {
    let head = self.head.load(Ordering::Relaxed);
    let tail = self.tail.load(Ordering::Relaxed);
    let capacity = self.capacity.load(Ordering::Relaxed);
    let len = if tail >= head {
      tail - head
    } else {
      capacity - head + tail
    };
    QueueSize::Limited(len)
  }

  async fn capacity(&self) -> QueueSize {
    QueueSize::Limited(self.capacity.load(Ordering::Relaxed))
  }
}

#[async_trait]
impl<E: Element> QueueReader<E> for RingQueue<E> {
  async fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    let head = self.head.load(Ordering::Relaxed);
    let tail = self.tail.load(Ordering::Relaxed);

    if head == tail {
      return Ok(None); // Queue is empty
    }

    let mut buffer = self.buffer.lock().await;
    let item = buffer[head].take();
    self.head.store((head + 1) % self.capacity.load(Ordering::Relaxed), Ordering::Relaxed);
    Ok(item)
  }

  async fn clean_up(&mut self) {
    let mut buffer = self.buffer.lock().await;
    buffer.iter_mut().for_each(|item| *item = None);
    self.head.store(0, Ordering::Relaxed);
    self.tail.store(0, Ordering::Relaxed);
  }
}

#[async_trait]
impl<E: Element> QueueWriter<E> for RingQueue<E> {
  async fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    if self.is_full() {
      if self.dynamic.load(Ordering::Relaxed) {
        self.resize().await;
      } else {
        return Err(QueueError::OfferError(element));
      }
    }

    let mut buffer = self.buffer.lock().await;
    let tail = self.tail.load(Ordering::Relaxed);
    buffer[tail] = Some(element);
    self.tail.store((tail + 1) % self.capacity.load(Ordering::Relaxed), Ordering::Relaxed);
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

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
    assert!(queue.offer(5).await.is_ok());  // This should trigger resize
    assert_eq!(queue.capacity().await.to_usize(), 9);

    // Add more elements to test the new capacity
    assert!(queue.offer(6).await.is_ok());
    assert!(queue.offer(7).await.is_ok());
    assert!(queue.offer(8).await.is_ok());
    assert!(queue.offer(9).await.is_ok());  // This should trigger another resize
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