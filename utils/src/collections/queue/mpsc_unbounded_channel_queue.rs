use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::collections::element::Element;
use crate::collections::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};
use async_trait::async_trait;
use tokio::sync::mpsc::error::{SendError, TryRecvError};
use parking_lot::Mutex;
use tokio::sync::mpsc;

#[derive(Debug)]
struct MpscUnboundedChannelQueueInner<E> {
  receiver: Mutex<mpsc::UnboundedReceiver<E>>,
  is_closed: AtomicBool,
}

#[derive(Debug, Clone)]
pub struct MpscUnboundedChannelQueue<E> {
  sender: mpsc::UnboundedSender<E>,
  inner: Arc<MpscUnboundedChannelQueueInner<E>>,
  count: Arc<AtomicUsize>,
}

impl<T> MpscUnboundedChannelQueue<T> {
  pub fn new() -> Self {
    let (sender, receiver) = mpsc::unbounded_channel();
    Self {
      sender,
      inner: Arc::new(MpscUnboundedChannelQueueInner {
        receiver: Mutex::new(receiver),
        is_closed: AtomicBool::new(false),
      }),
      count: Arc::new(AtomicUsize::new(0)),
    }
  }

  async fn try_recv(&self) -> Result<T, TryRecvError> {
    if self.inner.is_closed.load(Ordering::SeqCst) {
      return Err(TryRecvError::Disconnected);
    }
    let mut guard = self.inner.receiver.lock();
    guard.try_recv()
  }

  async fn send(&self, element: T) -> Result<(), SendError<T>> {
    if self.inner.is_closed.load(Ordering::SeqCst) {
      return Err(SendError(element));
    }
    match self.sender.send(element) {
      Ok(()) => Ok(()),
      Err(err) => {
        self.inner.is_closed.store(true, Ordering::SeqCst);
        Err(err)
      }
    }
  }

  fn increment_count(&self) {
    self.count.fetch_add(1, Ordering::SeqCst);
  }

  fn decrement_count(&self) {
    self
      .count
      .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| Some(current.saturating_sub(1)))
      .ok();
  }
}

#[async_trait]
impl<E: Element> QueueBase<E> for MpscUnboundedChannelQueue<E> {
  async fn len(&self) -> QueueSize {
    QueueSize::Limited(self.count.load(Ordering::SeqCst))
  }

  async fn capacity(&self) -> QueueSize {
    QueueSize::Limitless
  }
}

#[async_trait]
impl<E: Element> QueueWriter<E> for MpscUnboundedChannelQueue<E> {
  async fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    match self.send(element).await {
      Ok(_) => {
        self.increment_count();
        Ok(())
      }
      Err(SendError(err)) => Err(QueueError::OfferError(err)),
    }
  }
}

impl Default for MpscUnboundedChannelQueue<i32> {
  fn default() -> Self {
    Self::new()
  }
}

#[async_trait]
impl<E: Element> QueueReader<E> for MpscUnboundedChannelQueue<E> {
  async fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    match self.try_recv().await {
      Ok(element) => {
        self.decrement_count();
        Ok(Some(element))
      }
      Err(TryRecvError::Empty) => Ok(None),
      Err(TryRecvError::Disconnected) => Err(QueueError::<E>::PoolError),
    }
  }

  async fn clean_up(&mut self) {
    self.count.store(0, Ordering::SeqCst);
    self.inner.is_closed.store(true, Ordering::SeqCst);
    {
      let mut guard = self.inner.receiver.lock();
      guard.close();
    }
    self.sender.closed().await;
  }
}

#[cfg(test)]
mod tests;
