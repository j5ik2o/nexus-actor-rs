use super::traits::{MpscBackend, MpscHandle};
use crate::collections::{QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter};

/// Queue facade that operates on an [`MpscBackend`]
///
/// This queue implements the multi-producer, single-consumer (MPSC) pattern,
/// allowing multiple threads to add elements and a single thread to retrieve them.
#[derive(Debug)]
pub struct MpscQueue<S, T>
where
  S: MpscHandle<T>, {
  storage: S,
  _marker: core::marker::PhantomData<T>,
}

impl<S, T> MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Creates a new [`MpscQueue`] using the specified storage.
  ///
  /// # Arguments
  ///
  /// * `storage` - The backend storage for the queue
  ///
  /// # Returns
  ///
  /// A new [`MpscQueue`] instance
  pub fn new(storage: S) -> Self {
    Self {
      storage,
      _marker: core::marker::PhantomData,
    }
  }

  /// Gets a reference to the backend storage.
  ///
  /// # Returns
  ///
  /// An immutable reference to the storage
  pub fn storage(&self) -> &S {
    &self.storage
  }

  /// Extracts the backend storage from the queue.
  ///
  /// This method consumes the [`MpscQueue`] and transfers ownership to the storage.
  ///
  /// # Returns
  ///
  /// The backend storage
  pub fn into_storage(self) -> S {
    self.storage
  }

  /// Sets the capacity of the queue.
  ///
  /// # Arguments
  ///
  /// * `capacity` - The new capacity. `None` means unlimited.
  ///
  /// # Returns
  ///
  /// `true` if the capacity was successfully set, `false` otherwise
  pub fn set_capacity(&self, capacity: Option<usize>) -> bool {
    self.storage.backend().set_capacity(capacity)
  }

  /// Adds an element to the queue.
  ///
  /// Returns an error if the queue is full or closed.
  ///
  /// # Arguments
  ///
  /// * `element` - The element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - Element was successfully added
  /// * `Err(QueueError::Full(element))` - Queue is full
  /// * `Err(QueueError::Closed(element))` - Queue is closed
  pub fn offer(&self, element: T) -> Result<(), QueueError<T>> {
    self.storage.backend().try_send(element)
  }

  /// Retrieves an element from the queue.
  ///
  /// Returns `None` if the queue is empty. Returns an error if the queue is closed.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - Element was successfully retrieved
  /// * `Ok(None)` - Queue is empty
  /// * `Err(QueueError::Disconnected)` - Queue is closed
  pub fn poll(&self) -> Result<Option<T>, QueueError<T>> {
    self.storage.backend().try_recv()
  }

  /// Cleans up and closes the queue.
  ///
  /// After calling this method, subsequent `offer` operations will fail,
  /// and `poll` operations will return an error after retrieving remaining elements.
  pub fn clean_up(&self) {
    self.storage.backend().close();
  }

  /// Gets the capacity of the queue.
  ///
  /// # Returns
  ///
  /// The queue capacity. [`QueueSize::Unbounded`] if unlimited
  pub fn capacity(&self) -> QueueSize {
    self.storage.backend().capacity()
  }

  /// Checks whether the queue is closed.
  ///
  /// # Returns
  ///
  /// `true` if the queue is closed, `false` otherwise
  pub fn is_closed(&self) -> bool {
    self.storage.backend().is_closed()
  }

  /// Gets a reference to the backend (internal use).
  ///
  /// # Returns
  ///
  /// A reference to the backend
  fn backend(&self) -> &S::Backend {
    self.storage.backend()
  }
}

impl<S, T> Clone for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Creates a clone of the queue.
  ///
  /// The backend storage is shared, so the cloned queue references
  /// the same queue instance.
  ///
  /// # Returns
  ///
  /// A new [`MpscQueue`] instance sharing the same backend storage
  fn clone(&self) -> Self {
    Self {
      storage: self.storage.clone(),
      _marker: core::marker::PhantomData,
    }
  }
}

impl<S, T> QueueBase<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Gets the number of elements in the queue.
  ///
  /// # Returns
  ///
  /// The number of elements in the queue. [`QueueSize::Unbounded`] if unlimited
  fn len(&self) -> QueueSize {
    self.backend().len()
  }

  /// Gets the capacity of the queue.
  ///
  /// # Returns
  ///
  /// The queue capacity. [`QueueSize::Unbounded`] if unlimited
  fn capacity(&self) -> QueueSize {
    self.capacity()
  }
}

impl<S, T> QueueWriter<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Adds an element to the queue using a mutable reference.
  ///
  /// Returns an error if the queue is full or closed.
  ///
  /// # Arguments
  ///
  /// * `element` - The element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - Element was successfully added
  /// * `Err(QueueError::Full(element))` - Queue is full
  /// * `Err(QueueError::Closed(element))` - Queue is closed
  fn offer_mut(&mut self, element: T) -> Result<(), QueueError<T>> {
    self.backend().try_send(element)
  }
}

impl<S, T> QueueReader<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Retrieves an element from the queue using a mutable reference.
  ///
  /// Returns `None` if the queue is empty. Returns an error if the queue is closed.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - Element was successfully retrieved
  /// * `Ok(None)` - Queue is empty
  /// * `Err(QueueError::Disconnected)` - Queue is closed
  fn poll_mut(&mut self) -> Result<Option<T>, QueueError<T>> {
    self.backend().try_recv()
  }

  /// Cleans up and closes the queue using a mutable reference.
  ///
  /// After calling this method, subsequent `offer_mut` operations will fail,
  /// and `poll_mut` operations will return an error after retrieving remaining elements.
  fn clean_up_mut(&mut self) {
    self.backend().close();
  }
}

impl<S, T> QueueRw<T> for MpscQueue<S, T>
where
  S: MpscHandle<T>,
{
  /// Adds an element to the queue using a shared reference.
  ///
  /// Returns an error if the queue is full or closed.
  ///
  /// # Arguments
  ///
  /// * `element` - The element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - Element was successfully added
  /// * `Err(QueueError::Full(element))` - Queue is full
  /// * `Err(QueueError::Closed(element))` - Queue is closed
  fn offer(&self, element: T) -> Result<(), QueueError<T>> {
    self.offer(element)
  }

  /// Retrieves an element from the queue using a shared reference.
  ///
  /// Returns `None` if the queue is empty. Returns an error if the queue is closed.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(element))` - Element was successfully retrieved
  /// * `Ok(None)` - Queue is empty
  /// * `Err(QueueError::Disconnected)` - Queue is closed
  fn poll(&self) -> Result<Option<T>, QueueError<T>> {
    self.poll()
  }

  /// Cleans up and closes the queue using a shared reference.
  ///
  /// After calling this method, subsequent `offer` operations will fail,
  /// and `poll` operations will return an error after retrieving remaining elements.
  fn clean_up(&self) {
    self.clean_up();
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  use core::fmt;

  use crate::collections::queue::mpsc::backend::RingBufferBackend;
  use crate::collections::queue::mpsc::traits::MpscHandle;
  use crate::collections::queue::mpsc::{MpscBuffer, MpscQueue};
  use crate::collections::QueueError;

  struct RcBackendHandle<T>(Rc<RingBufferBackend<RefCell<MpscBuffer<T>>>>);

  impl<T> RcBackendHandle<T> {
    fn new(capacity: Option<usize>) -> Self {
      let buffer = RefCell::new(MpscBuffer::new(capacity));
      let backend = RingBufferBackend::new(buffer);
      Self(Rc::new(backend))
    }
  }

  impl<T> Clone for RcBackendHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> fmt::Debug for RcBackendHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      f.debug_struct("RcBackendHandle").finish()
    }
  }

  impl<T> core::ops::Deref for RcBackendHandle<T> {
    type Target = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> crate::sync::Shared<RingBufferBackend<RefCell<MpscBuffer<T>>>> for RcBackendHandle<T> {}

  impl<T> MpscHandle<T> for RcBackendHandle<T> {
    type Backend = RingBufferBackend<RefCell<MpscBuffer<T>>>;

    fn backend(&self) -> &Self::Backend {
      &self.0
    }
  }

  #[test]
  fn buffer_offer_and_poll() {
    let mut buffer: MpscBuffer<u32> = MpscBuffer::new(Some(1));
    assert!(buffer.offer(1).is_ok());
    assert!(matches!(buffer.offer(2), Err(QueueError::Full(2))));
    assert_eq!(buffer.poll().unwrap(), Some(1));
    assert!(buffer.poll().unwrap().is_none());
    buffer.clean_up();
    assert!(matches!(buffer.offer(3), Err(QueueError::Closed(3))));
  }

  #[test]
  fn shared_queue_shared_operations() {
    let queue: MpscQueue<_, u32> = MpscQueue::new(RcBackendHandle::<u32>::new(Some(2)));
    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert!(queue.offer(3).is_err());
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), Some(2));
  }

  #[test]
  fn shared_queue_cleanup_marks_closed() {
    let queue: MpscQueue<_, u32> = MpscQueue::new(RcBackendHandle::<u32>::new(None));
    queue.offer(1).unwrap();
    queue.clean_up();
    assert!(matches!(queue.poll(), Err(QueueError::Disconnected)));
    assert!(matches!(queue.offer(2), Err(QueueError::Closed(2))));
  }
}
