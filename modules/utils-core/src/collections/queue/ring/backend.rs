use crate::collections::queue::traits::{QueueBase, QueueReader, QueueWriter};
use crate::collections::queue::QueueStorage;
use crate::collections::{QueueError, QueueSize};
use crate::sync::Shared;

/// Backend abstraction trait for ring buffer-based queues.
///
/// This trait defines basic operations for queues using ring buffers.
/// Concrete implementations can use different synchronization mechanisms or persistence strategies.
///
/// # Type Parameters
///
/// * `E` - Type of elements stored in the queue
pub trait RingBackend<E> {
  /// Adds an element to the queue.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If element was successfully added
  /// * `Err(QueueError<E>)` - If queue is full or other errors occurred
  fn offer(&self, element: E) -> Result<(), QueueError<E>>;

  /// Removes an element from the queue.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - If element was successfully removed
  /// * `Ok(None)` - If queue is empty
  /// * `Err(QueueError<E>)` - If an error occurred
  fn poll(&self) -> Result<Option<E>, QueueError<E>>;

  /// Cleans up the queue's internal state.
  ///
  /// This method performs maintenance tasks such as releasing unused resources
  /// and optimizing internal buffers.
  fn clean_up(&self);

  /// Returns the number of elements currently stored in the queue.
  ///
  /// # Returns
  ///
  /// Queue size (`QueueSize::Limited(n)` or `QueueSize::Unlimited`)
  fn len(&self) -> QueueSize;

  /// Returns the queue capacity (maximum storable count).
  ///
  /// # Returns
  ///
  /// Queue capacity (`QueueSize::Limited(n)` or `QueueSize::Unlimited`)
  fn capacity(&self) -> QueueSize;

  /// Enables or disables the queue's dynamic resizing feature.
  ///
  /// # Arguments
  ///
  /// * `dynamic` - If `true`, enables dynamic resizing; if `false`, disables it.
  fn set_dynamic(&self, dynamic: bool);

  /// Checks if the queue is empty.
  ///
  /// # Returns
  ///
  /// * `true` - If queue is empty
  /// * `false` - If queue has one or more elements
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }
}

/// Handle trait that provides references to [`RingBackend`].
///
/// This trait defines handle types for managing shared references to ring buffer backends.
/// It is cloneable and ensures safe access from multiple threads.
///
/// # Type Parameters
///
/// * `E` - Type of elements stored in the queue
pub trait RingHandle<E>: Shared<Self::Backend> + Clone {
  /// Backend type referenced by this handle.
  ///
  /// `?Sized` allows dynamically-sized trait objects.
  type Backend: RingBackend<E> + ?Sized;

  /// Gets a reference to the backend.
  ///
  /// # Returns
  ///
  /// Immutable reference to the backend
  fn backend(&self) -> &Self::Backend;
}

/// Backend implementation that directly operates on ring buffer storage handles.
///
/// This struct provides a concrete backend implementation that directly accesses
/// ring buffer storage via [`QueueHandle`].
///
/// # Type Parameters
///
/// * `S` - Ring buffer storage handle type
///
/// [`QueueHandle`]: crate::collections::queue::QueueHandle
#[derive(Debug)]
pub struct RingStorageBackend<S> {
  storage: S,
}

impl<S> RingStorageBackend<S> {
  /// Creates a new `RingStorageBackend`.
  ///
  /// # Arguments
  ///
  /// * `storage` - Storage handle to use
  ///
  /// # Returns
  ///
  /// New `RingStorageBackend` instance
  pub const fn new(storage: S) -> Self {
    Self { storage }
  }

  /// Gets a reference to the storage handle.
  ///
  /// # Returns
  ///
  /// Immutable reference to the storage handle
  pub fn storage(&self) -> &S {
    &self.storage
  }

  /// Consumes this backend and returns the internal storage handle.
  ///
  /// # Returns
  ///
  /// Internal storage handle
  pub fn into_storage(self) -> S {
    self.storage
  }
}

/// [`RingBackend`] trait implementation for `RingStorageBackend`.
///
/// This implementation delegates all operations to the ring buffer via the storage handle.
/// Each operation is executed using appropriate read or write locks.
impl<S, E> RingBackend<E> for RingStorageBackend<S>
where
  S: crate::collections::queue::QueueHandle<E>,
{
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.storage.storage().with_write(|buffer| buffer.offer_mut(element))
  }

  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.storage.storage().with_write(|buffer| buffer.poll_mut())
  }

  fn clean_up(&self) {
    self.storage.storage().with_write(|buffer| buffer.clean_up_mut());
  }

  fn len(&self) -> QueueSize {
    self.storage.storage().with_read(|buffer| buffer.len())
  }

  fn capacity(&self) -> QueueSize {
    self.storage.storage().with_read(|buffer| buffer.capacity())
  }

  fn set_dynamic(&self, dynamic: bool) {
    self.storage.storage().with_write(|buffer| buffer.set_dynamic(dynamic));
  }
}
