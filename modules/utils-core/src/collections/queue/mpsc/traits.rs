use crate::collections::{QueueError, QueueSize};
use crate::sync::Shared;

/// Transport-oriented abstraction for queue backends.
pub trait MpscBackend<T> {
  fn try_send(&self, element: T) -> Result<(), QueueError<T>>;
  fn try_recv(&self) -> Result<Option<T>, QueueError<T>>;
  fn close(&self);
  fn len(&self) -> QueueSize;
  fn capacity(&self) -> QueueSize;
  fn is_closed(&self) -> bool;
  fn set_capacity(&self, capacity: Option<usize>) -> bool {
    let _ = capacity;
    false
  }
}

/// Shared handle that exposes a [`MpscBackend`].
pub trait MpscHandle<T>: Shared<Self::Backend> + Clone {
  type Backend: MpscBackend<T> + ?Sized;

  fn backend(&self) -> &Self::Backend;
}
