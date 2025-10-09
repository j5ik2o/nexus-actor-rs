use crate::collections::{QueueError, QueueSize};
use crate::sync::Shared;

/// Transport-oriented trait abstracting MPSC queue backends.
///
/// This trait defines the basic operations of a Multiple Producer Single Consumer (MPSC) queue.
/// It provides an abstract interface for asynchronously transmitting messages between
/// multiple senders and a single receiver.
pub trait MpscBackend<T> {
  /// Attempts to send an element to the queue (non-blocking).
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - Element was successfully added
  /// * `Err(QueueError)` - Queue is full, closed, or another error occurred
  fn try_send(&self, element: T) -> Result<(), QueueError<T>>;

  /// Attempts to receive an element from the queue (non-blocking).
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - Element was successfully received
  /// * `Ok(None)` - Queue is empty
  /// * `Err(QueueError)` - An error occurred
  fn try_recv(&self) -> Result<Option<T>, QueueError<T>>;

  /// Closes the queue.
  ///
  /// After closing, no new elements can be sent, but
  /// elements already in the queue can still be received.
  fn close(&self);

  /// Gets the number of elements currently in the queue.
  ///
  /// # Returns
  ///
  /// `QueueSize` representing the element count (bounded or unbounded)
  fn len(&self) -> QueueSize;

  /// Gets the capacity of the queue.
  ///
  /// # Returns
  ///
  /// `QueueSize` representing the capacity (bounded or unbounded)
  fn capacity(&self) -> QueueSize;

  /// Checks if the queue is closed.
  ///
  /// # Returns
  ///
  /// * `true` - Queue is closed
  /// * `false` - Queue is open
  fn is_closed(&self) -> bool;

  /// Sets the capacity of the queue.
  ///
  /// Default implementation does nothing and returns `false`.
  /// Backend implementations may override this method to support dynamic capacity changes.
  ///
  /// # Arguments
  ///
  /// * `capacity` - New capacity. `None` means unlimited
  ///
  /// # Returns
  ///
  /// * `true` - Capacity was successfully set
  /// * `false` - Capacity setting is not supported or failed
  fn set_capacity(&self, capacity: Option<usize>) -> bool {
    let _ = capacity;
    false
  }

  /// Checks if the queue is empty.
  ///
  /// # Returns
  ///
  /// * `true` - Queue is empty
  /// * `false` - Queue contains one or more elements
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }
}

/// Shared handle trait exposing an [`MpscBackend`].
///
/// This trait represents a shared handle that can be safely accessed from multiple threads.
/// By implementing `Shared` and `Clone`, the handle can be shared among multiple senders.
pub trait MpscHandle<T>: Shared<Self::Backend> + Clone {
  /// The backend type used by this handle.
  ///
  /// The backend must implement the [`MpscBackend`] trait.
  /// `?Sized` allows dynamic-sized backends (such as trait objects).
  type Backend: MpscBackend<T> + ?Sized;

  /// Gets a reference to the backend managed by this handle.
  ///
  /// # Returns
  ///
  /// Reference to the backend
  fn backend(&self) -> &Self::Backend;
}
