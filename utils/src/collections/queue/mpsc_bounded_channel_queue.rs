use std::fmt::Debug;
use std::sync::Arc;

use crate::collections::element::Element;
use crate::collections::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};
use async_trait::async_trait;
use tokio::sync::mpsc::error::{SendError, TryRecvError};
use tokio::sync::{mpsc, Mutex};

#[derive(Debug)]
struct MpscBoundedQueueInner<E> {
  receiver: mpsc::Receiver<E>,
  count: usize,
  capacity: usize,
  is_closed: bool,
}

#[derive(Debug, Clone)]
pub struct MpscBoundedChannelQueue<E> {
  sender: mpsc::Sender<E>,
  inner: Arc<Mutex<MpscBoundedQueueInner<E>>>,
}

impl<T> MpscBoundedChannelQueue<T> {
  pub fn new(buffer: usize) -> Self {
    let (sender, receiver) = mpsc::channel(buffer);
    Self {
      sender,
      inner: Arc::new(Mutex::new(MpscBoundedQueueInner {
        receiver,
        count: 0,
        capacity: buffer,
        is_closed: false,
      })),
    }
  }

  async fn try_recv(&self) -> Result<T, TryRecvError> {
    let mut inner_mg = self.inner.lock().await;
    if inner_mg.is_closed {
      return Err(TryRecvError::Disconnected);
    }
    inner_mg.receiver.try_recv()
  }

  async fn try_send(&self, element: T) -> Result<(), SendError<T>> {
    let inner_mg = self.inner.lock().await;
    if inner_mg.is_closed {
      return Err(SendError(element));
    }
    if inner_mg.count >= inner_mg.capacity {
      return Err(SendError(element));
    }
    drop(inner_mg); // Release the lock before sending
    match self.sender.try_send(element) {
      Ok(()) => Ok(()),
      Err(mpsc::error::TrySendError::Full(e)) => Err(SendError(e)),
      Err(mpsc::error::TrySendError::Closed(e)) => Err(SendError(e)),
    }
  }

  async fn increment_count(&self) {
    let mut inner_mg = self.inner.lock().await;
    inner_mg.count += 1;
  }

  async fn decrement_count(&self) {
    let mut inner_mg = self.inner.lock().await;
    inner_mg.count = inner_mg.count.saturating_sub(1);
  }
}

#[async_trait]
impl<E: Element> QueueBase<E> for MpscBoundedChannelQueue<E> {
  async fn len(&self) -> QueueSize {
    let inner_mg = self.inner.lock().await;
    QueueSize::Limited(inner_mg.count)
  }

  async fn capacity(&self) -> QueueSize {
    let inner_mg = self.inner.lock().await;
    QueueSize::Limited(inner_mg.capacity)
  }
}

#[async_trait]
impl<E: Element> QueueWriter<E> for MpscBoundedChannelQueue<E> {
  async fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    match self.try_send(element).await {
      Ok(_) => {
        self.increment_count().await;
        Ok(())
      }
      Err(SendError(err)) => Err(QueueError::OfferError(err)),
    }
  }
}

#[async_trait]
impl<E: Element> QueueReader<E> for MpscBoundedChannelQueue<E> {
  async fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    match self.try_recv().await {
      Ok(element) => {
        self.decrement_count().await;
        Ok(Some(element))
      }
      Err(TryRecvError::Empty) => Ok(None),
      Err(TryRecvError::Disconnected) => Err(QueueError::<E>::PoolError),
    }
  }

  async fn clean_up(&mut self) {
    let mut inner_mg = self.inner.lock().await;
    inner_mg.count = 0;
    inner_mg.receiver.close();
    inner_mg.is_closed = true;
  }
}
