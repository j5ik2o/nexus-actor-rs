use super::{queue_error::QueueError, queue_size::QueueSize, QueueStorage};
use crate::sync::Shared;

/// Common trait defining basic queue operations.
///
/// This trait provides basic functionality for retrieving queue size information.
/// It serves as the base trait for [`QueueWriter`], [`QueueReader`], and [`QueueRw`] traits.
///
/// # Type Parameters
///
/// * `E` - Type of elements stored in the queue
pub trait QueueBase<E> {
  /// Returns the current size of the queue.
  ///
  /// # Returns
  ///
  /// Returns the current number of elements stored in the queue as [`QueueSize`].
  fn len(&self) -> QueueSize;

  /// Returns the queue capacity.
  ///
  /// # Returns
  ///
  /// Returns the maximum number of elements the queue can hold as [`QueueSize`].
  /// Returns `QueueSize::Limitless` for unlimited queues.
  fn capacity(&self) -> QueueSize;

  /// Checks if the queue is empty.
  ///
  /// # Returns
  ///
  /// Returns `true` if the queue is empty, `false` if elements exist.
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }
}

/// Trait providing write operations to the queue.
///
/// This trait provides functionality to add elements to the queue using mutable references.
/// Used in single-threaded environments or situations where appropriate locks are already acquired.
///
/// # Type Parameters
///
/// * `E` - Type of elements stored in the queue
pub trait QueueWriter<E>: QueueBase<E> {
  /// Adds an element to the queue (mutable reference version).
  ///
  /// Attempts to add an element to the queue. Returns an error if the queue
  /// is full or closed.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If element was successfully added
  /// * `Err(QueueError)` - If addition failed (full, closed, etc.)
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>>;
}

/// Trait providing read operations from the queue.
///
/// This trait provides functionality to remove elements from the queue using mutable references.
/// Used in single-threaded environments or situations where appropriate locks are already acquired.
///
/// # Type Parameters
///
/// * `E` - Type of elements stored in the queue
pub trait QueueReader<E>: QueueBase<E> {
  /// Removes an element from the queue (mutable reference version).
  ///
  /// Removes and returns the front element of the queue. Returns `None` if the queue is empty.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - If element was successfully removed
  /// * `Ok(None)` - If queue is empty
  /// * `Err(QueueError)` - If read failed
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>>;

  /// Performs queue cleanup processing (mutable reference version).
  ///
  /// Organizes the queue's internal state and releases unnecessary resources.
  /// Depending on implementation, may perform memory optimization or buffer reorganization.
  fn clean_up_mut(&mut self);
}

/// Trait providing read/write operations for the queue.
///
/// This trait provides functionality for queue read/write operations using shared references.
/// Uses appropriate internal synchronization mechanisms for safe use in multithreaded environments.
///
/// # Type Parameters
///
/// * `E` - Type of elements stored in the queue
pub trait QueueRw<E>: QueueBase<E> {
  /// Adds an element to the queue (shared reference version).
  ///
  /// Attempts to add an element to the queue. Acquires appropriate internal locks
  /// to ensure thread-safe writes.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If element was successfully added
  /// * `Err(QueueError)` - If addition failed (full, closed, etc.)
  fn offer(&self, element: E) -> Result<(), QueueError<E>>;

  /// Removes an element from the queue (shared reference version).
  ///
  /// Removes and returns the front element of the queue. Acquires appropriate internal locks
  /// to ensure thread-safe reads.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - If element was successfully removed
  /// * `Ok(None)` - If queue is empty
  /// * `Err(QueueError)` - If read failed
  fn poll(&self) -> Result<Option<E>, QueueError<E>>;

  /// Performs queue cleanup processing (shared reference version).
  ///
  /// Organizes the queue's internal state and releases unnecessary resources.
  /// Acquires appropriate internal locks to ensure thread-safe cleanup.
  fn clean_up(&self);
}

/// Common interface for queue handles.
///
/// This trait defines handles for abstracting queue implementations and
/// uniformly handling different storage backends.
/// Inherits [`Shared`] trait and [`Clone`], ensuring safe queue access
/// from multiple threads.
///
/// # Type Parameters
///
/// * `E` - Type of elements stored in the queue
///
/// # Associated Types
///
/// * `Storage` - Type of storage used internally. Must implement [`QueueStorage`].
pub trait QueueHandle<E>: Shared<Self::Storage> + Clone {
  /// Storage backend type.
  ///
  /// Defines the internal storage type used by this queue.
  /// Must implement the [`QueueStorage`] trait.
  type Storage: QueueStorage<E> + ?Sized;

  /// Gets a reference to the internal storage.
  ///
  /// # Returns
  ///
  /// Returns a reference to the storage managed by this handle.
  fn storage(&self) -> &Self::Storage;
}
