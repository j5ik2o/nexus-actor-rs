use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::collections::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};
use crate::collections::Element;
use async_trait::async_trait;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct RingQueue<E> {
  inner: Arc<Inner<E>>,
}

#[derive(Debug)]
struct Inner<E> {
  buffer: Mutex<Vec<Option<E>>>,
  head: AtomicUsize,
  tail: AtomicUsize,
  capacity: AtomicUsize,
  dynamic: AtomicBool,
}

impl<E> RingQueue<E> {
  pub fn new(capacity: usize) -> Self {
    assert!(capacity > 0, "Capacity must be greater than zero");
    let mut buffer = Vec::with_capacity(capacity);
    buffer.resize_with(capacity, || None);
    Self {
      inner: Arc::new(Inner {
        buffer: Mutex::new(buffer),
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
        capacity: AtomicUsize::new(capacity),
        dynamic: AtomicBool::new(true),
      }),
    }
  }

  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.inner.dynamic.store(dynamic, Ordering::SeqCst);
    self
  }

  fn is_full(&self) -> bool {
    let head = self.inner.head.load(Ordering::SeqCst);
    let tail = self.inner.tail.load(Ordering::SeqCst);
    let capacity = self.inner.capacity.load(Ordering::SeqCst);
    (tail + 1) % capacity == head
  }

  async fn resize(&self) {
    let mut buffer = self.inner.buffer.lock().await;
    let old_capacity = buffer.len();
    let new_capacity = old_capacity * 2 + 1; // +1 to ensure odd capacity
    let mut new_buffer = Vec::with_capacity(new_capacity);
    new_buffer.resize_with(new_capacity, || None);

    let head = self.inner.head.load(Ordering::SeqCst);
    let tail = self.inner.tail.load(Ordering::SeqCst);

    let mut new_index = 0;
    let mut current = head;
    while current != tail {
      new_buffer[new_index] = buffer[current].take();
      current = (current + 1) % old_capacity;
      new_index += 1;
    }

    *buffer = new_buffer;
    self.inner.capacity.store(new_capacity, Ordering::SeqCst);
    self.inner.head.store(0, Ordering::SeqCst);
    self.inner.tail.store(new_index, Ordering::SeqCst);
  }
}

#[async_trait]
impl<E: Element> QueueBase<E> for RingQueue<E> {
  async fn is_empty(&self) -> bool {
    self.inner.head.load(Ordering::SeqCst) == self.inner.tail.load(Ordering::SeqCst)
  }

  async fn is_full(&self) -> bool {
    self.is_full()
  }

  async fn len(&self) -> QueueSize {
    let head = self.inner.head.load(Ordering::SeqCst);
    let tail = self.inner.tail.load(Ordering::SeqCst);
    let capacity = self.inner.capacity.load(Ordering::SeqCst);
    let len = if tail >= head {
      tail - head
    } else {
      capacity - head + tail
    };
    QueueSize::Limited(len)
  }

  async fn capacity(&self) -> QueueSize {
    QueueSize::Limited(self.inner.capacity.load(Ordering::SeqCst))
  }
}

#[async_trait]
impl<E: Element> QueueReader<E> for RingQueue<E> {
  async fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    let mut buffer = self.inner.buffer.lock().await;
    let head = self.inner.head.load(Ordering::SeqCst);
    let tail = self.inner.tail.load(Ordering::SeqCst);

    if head == tail {
      return Ok(None); // Queue is empty
    }

    let item = buffer[head].take();
    self.inner.head.store((head + 1) % buffer.len(), Ordering::SeqCst);
    Ok(item)
  }

  async fn clean_up(&mut self) {
    let mut buffer = self.inner.buffer.lock().await;
    buffer.iter_mut().for_each(|item| *item = None);
    self.inner.head.store(0, Ordering::SeqCst);
    self.inner.tail.store(0, Ordering::SeqCst);
  }
}

#[async_trait]
impl<E: Element> QueueWriter<E> for RingQueue<E> {
  async fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    if self.is_full() {
      if self.inner.dynamic.load(Ordering::SeqCst) {
        self.resize().await;
      } else {
        return Err(QueueError::OfferError(element));
      }
    }

    let mut buffer = self.inner.buffer.lock().await;
    let tail = self.inner.tail.load(Ordering::SeqCst);
    buffer[tail] = Some(element);
    self.inner.tail.store((tail + 1) % buffer.len(), Ordering::SeqCst);
    Ok(())
  }
}

#[cfg(test)]
mod tests;
