use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::{TryRecvError, TrySendError};

use nexus_utils_core_rs::{Element, QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, SharedQueue};

#[derive(Debug)]
struct MpscBoundedInner<E> {
  receiver: Mutex<mpsc::Receiver<E>>,
  capacity: usize,
  is_closed: AtomicBool,
}

#[derive(Debug, Clone)]
pub struct ArcMpscBoundedQueue<E> {
  sender: mpsc::Sender<E>,
  inner: Arc<MpscBoundedInner<E>>,
  len: Arc<AtomicUsize>,
}

impl<E> ArcMpscBoundedQueue<E> {
  pub fn new(capacity: usize) -> Self {
    let (sender, receiver) = mpsc::channel(capacity);
    Self {
      sender,
      inner: Arc::new(MpscBoundedInner {
        receiver: Mutex::new(receiver),
        capacity,
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

  fn try_send(&self, element: E) -> Result<(), TrySendError<E>> {
    if self.inner.is_closed.load(Ordering::SeqCst) {
      return Err(TrySendError::Closed(element));
    }
    self.sender.try_send(element)
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
    match self.try_send(element) {
      Ok(()) => {
        self.increment_len();
        Ok(())
      }
      Err(TrySendError::Full(item)) => Err(QueueError::Full(item)),
      Err(TrySendError::Closed(item)) => {
        self.inner.is_closed.store(true, Ordering::SeqCst);
        Err(QueueError::Closed(item))
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

  pub fn capacity_shared(&self) -> QueueSize {
    QueueSize::limited(self.inner.capacity)
  }
}

impl<E: Element> QueueBase<E> for ArcMpscBoundedQueue<E> {
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    self.capacity_shared()
  }
}

impl<E: Element> QueueWriter<E> for ArcMpscBoundedQueue<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<E: Element> QueueReader<E> for ArcMpscBoundedQueue<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&mut self) {
    self.clean_up_shared();
  }
}

impl<E: Element> SharedQueue<E> for ArcMpscBoundedQueue<E> {
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn bounded_queue_respects_capacity() {
    let queue = ArcMpscBoundedQueue::new(1);
    queue.offer_shared(1).unwrap();
    let err = queue.offer_shared(2).unwrap_err();
    assert!(matches!(err, QueueError::Full(2)));
  }

  #[test]
  fn bounded_queue_poll_returns_items() {
    let queue = ArcMpscBoundedQueue::new(2);
    queue.offer_shared(1).unwrap();
    queue.offer_shared(2).unwrap();

    assert_eq!(queue.len().to_usize(), 2);
    assert_eq!(queue.poll_shared().unwrap(), Some(1));
    assert_eq!(queue.poll_shared().unwrap(), Some(2));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }

  #[test]
  fn bounded_queue_clean_up_closes_channel() {
    let queue = ArcMpscBoundedQueue::new(1);
    queue.offer_shared(1).unwrap();
    queue.clean_up_shared();

    assert!(matches!(queue.poll_shared(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer_shared(2), Err(QueueError::Closed(2))));
  }

  #[test]
  fn bounded_queue_capacity_and_len_shared() {
    let queue = ArcMpscBoundedQueue::new(3);
    assert_eq!(queue.capacity_shared(), QueueSize::limited(3));
    queue.offer_shared(1).unwrap();
    assert_eq!(queue.len_shared(), QueueSize::limited(1));
  }
}
