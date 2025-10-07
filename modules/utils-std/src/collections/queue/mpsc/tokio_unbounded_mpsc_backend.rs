use std::fmt;

use nexus_utils_core_rs::{Element, MpscBackend, QueueError, QueueSize};
use parking_lot::Mutex;
use tokio::sync::mpsc;

/// Tokio-based backend for unbounded MPSC queues.
pub struct TokioUnboundedMpscBackend<T> {
  sender: mpsc::UnboundedSender<T>,
  receiver: Mutex<mpsc::UnboundedReceiver<T>>,
}

impl<T> Default for TokioUnboundedMpscBackend<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TokioUnboundedMpscBackend<T> {
  pub fn new() -> Self {
    let (sender, receiver) = mpsc::unbounded_channel();
    Self {
      sender,
      receiver: Mutex::new(receiver),
    }
  }

  fn with_receiver<R>(&self, f: impl FnOnce(&mut mpsc::UnboundedReceiver<T>) -> R) -> R {
    let mut guard = self.receiver.lock();
    f(&mut guard)
  }
}

impl<T> fmt::Debug for TokioUnboundedMpscBackend<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("TokioUnboundedMpscBackend").finish()
  }
}

impl<T> MpscBackend<T> for TokioUnboundedMpscBackend<T>
where
  T: Element,
{
  fn try_send(&self, element: T) -> Result<(), QueueError<T>> {
    self.sender.send(element).map_err(|error| QueueError::Closed(error.0))
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
    QueueSize::limitless()
  }

  fn is_closed(&self) -> bool {
    self.sender.is_closed()
  }

  fn set_capacity(&self, _capacity: Option<usize>) -> bool {
    false
  }
}
