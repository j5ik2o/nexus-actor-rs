use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc::error::{SendError, TryRecvError};
use tokio::sync::{mpsc, Mutex};

use crate::util::element::Element;
use crate::util::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

#[derive(Debug)]
struct MpscBoundedQueueInner<E> {
  rx: mpsc::Receiver<E>,
  count: usize,
  capacity: usize,
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
        rx: receiver,
        count: 0,
        capacity: buffer,
      })),
    }
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
    match self.sender.send(element).await {
      Ok(_) => {
        let mut inner_mg = self.inner.lock().await;
        inner_mg.count += 1;
        Ok(())
      }
      Err(SendError(err)) => Err(QueueError::OfferError(err)),
    }
  }
}

#[async_trait]
impl<E: Element> QueueReader<E> for MpscBoundedChannelQueue<E> {
  async fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    match {
      let mut inner_mg = self.inner.lock().await;
      inner_mg.rx.try_recv()
    } {
      Ok(element) => {
        let mut inner_mg = self.inner.lock().await;
        inner_mg.count = inner_mg.count.saturating_sub(1);
        Ok(Some(element))
      }
      Err(TryRecvError::Empty) => Ok(None),
      Err(TryRecvError::Disconnected) => Err(QueueError::<E>::PoolError),
    }
  }

  async fn clean_up(&mut self) {
    let mut inner_mg = self.inner.lock().await;
    inner_mg.count = 0;
    inner_mg.rx.close();
  }
}
