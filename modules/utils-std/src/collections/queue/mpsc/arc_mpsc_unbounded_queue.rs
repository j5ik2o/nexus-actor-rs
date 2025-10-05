use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

use nexus_utils_core_rs::{Element, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedQueue};

#[derive(Debug)]
struct MpscUnboundedInner<E> {
  receiver: Mutex<mpsc::UnboundedReceiver<E>>,
  is_closed: AtomicBool,
}

#[derive(Debug, Clone)]
pub struct ArcMpscUnboundedQueue<E> {
  sender: mpsc::UnboundedSender<E>,
  inner: Arc<MpscUnboundedInner<E>>,
  len: Arc<AtomicUsize>,
}

impl<E> ArcMpscUnboundedQueue<E> {
  pub fn new() -> Self {
    let (sender, receiver) = mpsc::unbounded_channel();
    Self {
      sender,
      inner: Arc::new(MpscUnboundedInner {
        receiver: Mutex::new(receiver),
        is_closed: AtomicBool::new(false),
      }),
      len: Arc::new(AtomicUsize::new(0)),
    }
  }

  fn try_recv(&self) -> Result<E, TryRecvError> {
    if self.inner.is_closed.load(Ordering::SeqCst) {
      return Err(TryRecvError::Disconnected);
    }
    let mut guard = self.inner.receiver.lock();
    guard.try_recv()
  }

  fn increment_len(&self) {
    self.len.fetch_add(1, Ordering::SeqCst);
  }

  fn decrement_len(&self) {
    self
      .len
      .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
        Some(current.saturating_sub(1))
      })
      .ok();
  }

  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>>
  where
    E: Element, {
    match self.sender.send(element) {
      Ok(()) => {
        self.increment_len();
        Ok(())
      }
      Err(err) => {
        self.inner.is_closed.store(true, Ordering::SeqCst);
        Err(QueueError::Closed(err.0))
      }
    }
  }

  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>>
  where
    E: Element, {
    match self.try_recv() {
      Ok(value) => {
        self.decrement_len();
        Ok(Some(value))
      }
      Err(TryRecvError::Empty) => Ok(None),
      Err(TryRecvError::Disconnected) => {
        self.inner.is_closed.store(true, Ordering::SeqCst);
        Err(QueueError::Disconnected)
      }
    }
  }

  pub fn clean_up_shared(&self) {
    self.len.store(0, Ordering::SeqCst);
    self.inner.is_closed.store(true, Ordering::SeqCst);
    let mut guard = self.inner.receiver.lock();
    guard.close();
  }

  pub fn len_shared(&self) -> QueueSize {
    QueueSize::limited(self.len.load(Ordering::SeqCst))
  }
}

impl<E: Element> QueueBase<E> for ArcMpscUnboundedQueue<E> {
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    QueueSize::limitless()
  }
}

impl<E: Element> QueueWriter<E> for ArcMpscUnboundedQueue<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E: Element> QueueReader<E> for ArcMpscUnboundedQueue<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&mut self) {
    self.clean_up_shared();
  }
}

impl<E: Element> SharedQueue<E> for ArcMpscUnboundedQueue<E> {
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }

  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up_shared(&self) {
    self.clean_up_shared();
  }
}

impl<E: Element> Default for ArcMpscUnboundedQueue<E> {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn unbounded_queue_offer_poll_cycle() {
    let queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    queue.offer_shared(10).unwrap();
    queue.offer_shared(20).unwrap();

    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll_shared().unwrap(), Some(10));
    assert_eq!(queue.poll_shared().unwrap(), Some(20));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }

  #[test]
  fn unbounded_queue_len_shared_updates() {
    let queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    queue.offer_shared(1).unwrap();
    assert_eq!(queue.len_shared(), QueueSize::limited(1));
  }

  #[test]
  fn unbounded_queue_closed_on_receiver_drop() {
    let queue: ArcMpscUnboundedQueue<u32> = ArcMpscUnboundedQueue::new();
    queue.clean_up_shared();
    let err = queue.poll_shared().unwrap_err();
    assert!(matches!(err, QueueError::Disconnected));
    assert!(matches!(queue.offer_shared(1), Err(QueueError::Closed(1))));
  }
}
