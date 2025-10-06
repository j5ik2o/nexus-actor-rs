use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::collections::Element;
use crate::collections::{QueueBase, QueueReader, QueueSupport, QueueWriter};
use crate::collections::{QueueError, QueueSize};
use nexus_utils_core_rs::collections::RingBuffer;
use parking_lot::Mutex;

#[derive(Debug, Clone)]
pub struct RingQueue<E> {
  inner: Arc<Inner<E>>,
}

#[derive(Debug)]
struct Inner<E> {
  buffer: Mutex<RingBuffer<E>>,
  dynamic: AtomicBool,
}

impl<E: Element> RingQueue<E> {
  pub fn new(capacity: usize) -> Self {
    Self {
      inner: Arc::new(Inner {
        buffer: Mutex::new(RingBuffer::new(capacity)),
        dynamic: AtomicBool::new(true),
      }),
    }
  }

  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.inner.dynamic.store(dynamic, Ordering::SeqCst);
    if let Some(mut guard) = self.inner.buffer.try_lock() {
      guard.set_dynamic(dynamic);
    }
    self
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    let dynamic = self.inner.dynamic.load(Ordering::SeqCst);
    self.with_lock(|buffer| {
      buffer.set_dynamic(dynamic);
      buffer.offer(element)
    })
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.with_lock(|buffer| buffer.poll())
  }

  pub fn clean_up_shared(&self) {
    self.with_lock(|buffer| buffer.clean_up());
  }

  pub fn len_shared(&self) -> usize {
    self.with_lock(|buffer| buffer.len().to_usize())
  }

  fn with_lock<F, R>(&self, f: F) -> R
  where
    F: FnOnce(&mut RingBuffer<E>) -> R, {
    let mut guard = self.inner.buffer.lock();
    f(&mut guard)
  }
}

impl<E: Element> QueueBase<E> for RingQueue<E> {
  fn len(&self) -> QueueSize {
    self.with_lock(|buffer| buffer.len())
  }

  fn capacity(&self) -> QueueSize {
    self.with_lock(|buffer| buffer.capacity())
  }
}

impl<E: Element> QueueWriter<E> for RingQueue<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    let dynamic = self.inner.dynamic.load(Ordering::SeqCst);
    self.with_lock(|buffer| {
      buffer.set_dynamic(dynamic);
      buffer.offer(element)
    })
  }
}

impl<E: Element> QueueReader<E> for RingQueue<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.with_lock(|buffer| buffer.poll())
  }

  fn clean_up(&mut self) {
    self.with_lock(|buffer| buffer.clean_up());
  }
}

impl<E: Element> QueueSupport for RingQueue<E> {}

#[cfg(test)]
mod tests;
