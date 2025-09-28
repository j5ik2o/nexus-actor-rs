use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::collections::element::Element;
use crate::collections::queue_sync::{SyncQueueBase, SyncQueueReader, SyncQueueSupport, SyncQueueWriter};
use crate::collections::{QueueError, QueueSize};
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::{SendError, TryRecvError};

#[derive(Debug)]
struct MpscBoundedQueueInner<E> {
  receiver: Mutex<mpsc::Receiver<E>>,
  capacity: usize,
  is_closed: AtomicBool,
}

#[derive(Debug, Clone)]
pub struct MpscBoundedChannelQueue<E> {
  sender: mpsc::Sender<E>,
  inner: Arc<MpscBoundedQueueInner<E>>,
  count: Arc<AtomicUsize>,
}

impl<T> MpscBoundedChannelQueue<T> {
  pub fn new(buffer: usize) -> Self {
    let (sender, receiver) = mpsc::channel(buffer);
    Self {
      sender,
      inner: Arc::new(MpscBoundedQueueInner {
        receiver: Mutex::new(receiver),
        capacity: buffer,
        is_closed: AtomicBool::new(false),
      }),
      count: Arc::new(AtomicUsize::new(0)),
    }
  }

  fn try_recv(&self) -> Result<T, TryRecvError> {
    if self.inner.is_closed.load(Ordering::SeqCst) {
      return Err(TryRecvError::Disconnected);
    }
    let mut guard = self.inner.receiver.lock();
    guard.try_recv()
  }

  fn try_send(&self, element: T) -> Result<(), SendError<T>> {
    if self.inner.is_closed.load(Ordering::SeqCst) {
      return Err(SendError(element));
    }
    match self.sender.try_send(element) {
      Ok(()) => Ok(()),
      Err(mpsc::error::TrySendError::Full(e)) => Err(SendError(e)),
      Err(mpsc::error::TrySendError::Closed(e)) => {
        self.inner.is_closed.store(true, Ordering::SeqCst);
        Err(SendError(e))
      }
    }
  }

  fn increment_count(&self) {
    self.count.fetch_add(1, Ordering::SeqCst);
  }

  fn decrement_count(&self) {
    self
      .count
      .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
        Some(current.saturating_sub(1))
      })
      .ok();
  }
}

impl<E: Element> SyncQueueBase<E> for MpscBoundedChannelQueue<E> {
  fn len(&self) -> QueueSize {
    QueueSize::Limited(self.count.load(Ordering::SeqCst))
  }

  fn capacity(&self) -> QueueSize {
    QueueSize::Limited(self.inner.capacity)
  }
}

impl<E: Element> SyncQueueWriter<E> for MpscBoundedChannelQueue<E> {
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    match self.try_send(element) {
      Ok(_) => {
        self.increment_count();
        Ok(())
      }
      Err(SendError(err)) => Err(QueueError::OfferError(err)),
    }
  }
}

impl<E: Element> SyncQueueReader<E> for MpscBoundedChannelQueue<E> {
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    match self.try_recv() {
      Ok(element) => {
        self.decrement_count();
        Ok(Some(element))
      }
      Err(TryRecvError::Empty) => Ok(None),
      Err(TryRecvError::Disconnected) => Err(QueueError::<E>::PoolError),
    }
  }

  fn clean_up(&mut self) {
    self.count.store(0, Ordering::SeqCst);
    self.inner.is_closed.store(true, Ordering::SeqCst);
    let mut guard = self.inner.receiver.lock();
    guard.close();
  }
}

impl<E: Element> SyncQueueSupport for MpscBoundedChannelQueue<E> {}

#[cfg(test)]
mod tests;
