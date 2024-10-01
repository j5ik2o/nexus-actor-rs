use std::fmt::Debug;
use std::sync::Arc;

use crate::collections::element::Element;
use crate::collections::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};
use async_trait::async_trait;
use tokio::sync::mpsc::error::{SendError, TryRecvError};
use tokio::sync::{mpsc, Mutex};

#[derive(Debug)]
struct MpscUnboundedChannelQueueInner<E> {
  receiver: mpsc::UnboundedReceiver<E>,
  count: usize,
  is_closed: bool,
}

#[derive(Debug, Clone)]
pub struct MpscUnboundedChannelQueue<E> {
  sender: mpsc::UnboundedSender<E>,
  inner: Arc<Mutex<MpscUnboundedChannelQueueInner<E>>>,
}

impl<T> MpscUnboundedChannelQueue<T> {
  pub fn new() -> Self {
    let (sender, receiver) = mpsc::unbounded_channel();
    Self {
      sender,
      inner: Arc::new(Mutex::new(MpscUnboundedChannelQueueInner {
        receiver,
        count: 0,
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

  async fn send(&self, element: T) -> Result<(), SendError<T>> {
    let inner_mg = self.inner.lock().await;
    if inner_mg.is_closed {
      return Err(SendError(element));
    }
    drop(inner_mg);
    self.sender.send(element)
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
impl<E: Element> QueueBase<E> for MpscUnboundedChannelQueue<E> {
  async fn len(&self) -> QueueSize {
    let inner_mg = self.inner.lock().await;
    QueueSize::Limited(inner_mg.count)
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
        self.increment_count().await;
        Ok(())
      }
      Err(SendError(err)) => Err(QueueError::OfferError(err)),
    }
  }
}

#[async_trait]
impl<E: Element> QueueReader<E> for MpscUnboundedChannelQueue<E> {
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
    drop(inner_mg);
    self.sender.closed().await;
  }
}
