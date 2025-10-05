use std::fmt;

use nexus_utils_core_rs::{Element, MpscBackend, QueueError, QueueSize};
use parking_lot::Mutex;
use tokio::sync::mpsc;

/// Tokio-based backend for bounded MPSC queues.
pub struct TokioBoundedMpscBackend<T> {
  sender: mpsc::Sender<T>,
  receiver: Mutex<mpsc::Receiver<T>>,
  capacity: usize,
}

impl<T> TokioBoundedMpscBackend<T> {
  pub fn new(capacity: usize) -> Self {
    let (sender, receiver) = mpsc::channel(capacity);
    Self {
      sender,
      receiver: Mutex::new(receiver),
      capacity,
    }
  }

  fn with_receiver<R>(&self, f: impl FnOnce(&mut mpsc::Receiver<T>) -> R) -> R {
    let mut guard = self.receiver.lock();
    f(&mut guard)
  }
}

impl<T> fmt::Debug for TokioBoundedMpscBackend<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("TokioBoundedMpscBackend")
      .field("capacity", &self.capacity)
      .finish()
  }
}

impl<T> MpscBackend<T> for TokioBoundedMpscBackend<T>
where
  T: Element,
{
  fn try_send(&self, element: T) -> Result<(), QueueError<T>> {
    match self.sender.try_send(element) {
      Ok(()) => Ok(()),
      Err(mpsc::error::TrySendError::Closed(value)) => Err(QueueError::Closed(value)),
      Err(mpsc::error::TrySendError::Full(value)) => Err(QueueError::Full(value)),
    }
  }

  fn try_recv(&self) -> Result<Option<T>, QueueError<T>> {
    self.with_receiver(|receiver| match receiver.try_recv() {
      Ok(value) => Ok(Some(value)),
      Err(mpsc::error::TryRecvError::Empty) => {
        if self.sender.is_closed() {
          Err(QueueError::Disconnected)
        } else {
          Ok(None)
        }
      }
      Err(mpsc::error::TryRecvError::Disconnected) => Err(QueueError::Disconnected),
    })
  }

  fn close(&self) {
    self.with_receiver(|receiver| {
      receiver.close();
      while receiver.try_recv().is_ok() {}
    });
  }

  fn len(&self) -> QueueSize {
    self.with_receiver(|receiver| QueueSize::limited(receiver.len()))
  }

  fn capacity(&self) -> QueueSize {
    QueueSize::limited(self.capacity)
  }

  fn is_closed(&self) -> bool {
    self.sender.is_closed()
  }

  fn set_capacity(&self, _capacity: Option<usize>) -> bool {
    false
  }
}
