use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc::error::{SendError, TryRecvError};
use tokio::sync::{mpsc, Mutex};

use crate::util::element::Element;
use crate::util::queue::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter};

#[derive(Debug)]
struct MpscUnboundedChannelQueueInner<E> {
  rx: mpsc::UnboundedReceiver<E>,
  count: usize,
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
      inner: Arc::new(Mutex::new(MpscUnboundedChannelQueueInner { rx: receiver, count: 0 })),
    }
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
    match self.sender.send(element) {
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
impl<E: Element> QueueReader<E> for MpscUnboundedChannelQueue<E> {
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
