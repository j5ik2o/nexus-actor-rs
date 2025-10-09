use core::marker::PhantomData;

use super::backend::{RingBackend, RingHandle};
use crate::collections::queue::{QueueBase, QueueError, QueueReader, QueueRw, QueueSize, QueueWriter};

/// Queue facade that delegates all operations to a [`RingBackend`].
#[derive(Debug)]
pub struct RingQueue<H, E>
where
  H: RingHandle<E>, {
  backend: H,
  _marker: PhantomData<E>,
}

impl<H, E> RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Creates a new `RingQueue` from the specified backend handle.
  ///
  /// # Arguments
  ///
  /// * `backend` - Backend handle for the ring queue
  ///
  /// # Returns
  ///
  /// New `RingQueue` instance
  pub fn new(backend: H) -> Self {
    Self {
      backend,
      _marker: PhantomData,
    }
  }

  /// Returns a reference to the backend handle.
  ///
  /// # Returns
  ///
  /// Immutable reference to the backend handle
  pub fn backend(&self) -> &H {
    &self.backend
  }

  /// Consumes this queue and returns the internal backend handle.
  ///
  /// # Returns
  ///
  /// Internal backend handle
  pub fn into_backend(self) -> H {
    self.backend
  }

  /// Sets the dynamic mode of the queue.
  ///
  /// When dynamic mode is enabled, the queue automatically expands when elements are added beyond capacity.
  ///
  /// # Arguments
  ///
  /// * `dynamic` - If `true`, enables dynamic mode
  pub fn set_dynamic(&self, dynamic: bool) {
    self.backend.backend().set_dynamic(dynamic);
  }

  /// Builder method that sets dynamic mode and returns this queue.
  ///
  /// When dynamic mode is enabled, the queue automatically expands when elements are added beyond capacity.
  ///
  /// # Arguments
  ///
  /// * `dynamic` - If `true`, enables dynamic mode
  ///
  /// # Returns
  ///
  /// This queue with dynamic mode set
  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  /// Adds an element to the queue.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If element was successfully added
  /// * `Err(QueueError)` - If queue is full and element cannot be added
  ///
  /// # Errors
  ///
  /// Returns `QueueError::Full` if the queue is full and dynamic mode is disabled.
  pub fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.backend.backend().offer(element)
  }

  /// Removes an element from the queue.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - Element removed from the queue
  /// * `Ok(None)` - If queue is empty
  /// * `Err(QueueError)` - If an error occurred
  pub fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.backend.backend().poll()
  }

  /// Performs queue cleanup.
  ///
  /// Performs maintenance such as internal buffer memory optimization.
  pub fn clean_up(&self) {
    self.backend.backend().clean_up();
  }
}

impl<H, E> Clone for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Creates a clone of the `RingQueue`.
  ///
  /// Clones the backend handle and returns a new `RingQueue` instance.
  ///
  /// # Returns
  ///
  /// Clone of this queue
  fn clone(&self) -> Self {
    Self {
      backend: self.backend.clone(),
      _marker: PhantomData,
    }
  }
}

impl<H, E> QueueBase<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Returns the number of elements in the queue.
  ///
  /// # Returns
  ///
  /// Current number of elements in the queue
  fn len(&self) -> QueueSize {
    self.backend.backend().len()
  }

  /// Returns the capacity of the queue.
  ///
  /// # Returns
  ///
  /// Maximum number of elements the queue can hold
  fn capacity(&self) -> QueueSize {
    self.backend.backend().capacity()
  }
}

impl<H, E> QueueWriter<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Adds an element to the queue using a mutable reference.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If element was successfully added
  /// * `Err(QueueError)` - If queue is full and element cannot be added
  fn offer_mut(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }
}

impl<H, E> QueueReader<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Removes an element from the queue using a mutable reference.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - Element removed from the queue
  /// * `Ok(None)` - If queue is empty
  /// * `Err(QueueError)` - If an error occurred
  fn poll_mut(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  /// Performs queue cleanup using a mutable reference.
  ///
  /// Performs maintenance such as internal buffer memory optimization.
  fn clean_up_mut(&mut self) {
    self.clean_up();
  }
}

impl<H, E> QueueRw<E> for RingQueue<H, E>
where
  H: RingHandle<E>,
{
  /// Adds an element to the queue using a shared reference.
  ///
  /// # Arguments
  ///
  /// * `element` - Element to add to the queue
  ///
  /// # Returns
  ///
  /// * `Ok(())` - If element was successfully added
  /// * `Err(QueueError)` - If queue is full and element cannot be added
  fn offer(&self, element: E) -> Result<(), QueueError<E>> {
    self.offer(element)
  }

  /// Removes an element from the queue using a shared reference.
  ///
  /// # Returns
  ///
  /// * `Ok(Some(E))` - Element removed from the queue
  /// * `Ok(None)` - If queue is empty
  /// * `Err(QueueError)` - If an error occurred
  fn poll(&self) -> Result<Option<E>, QueueError<E>> {
    self.poll()
  }

  /// Performs queue cleanup using a shared reference.
  ///
  /// Performs maintenance such as internal buffer memory optimization.
  fn clean_up(&self) {
    self.clean_up();
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;

  use alloc::rc::Rc;
  use core::cell::RefCell;

  use super::super::{RingBuffer, RingStorageBackend};
  use super::RingQueue;
  use crate::collections::queue::ring::backend::RingHandle;
  use crate::collections::queue::QueueHandle;

  struct RcStorageHandle<E>(Rc<RefCell<RingBuffer<E>>>);

  impl<E> Clone for RcStorageHandle<E> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<E> core::ops::Deref for RcStorageHandle<E> {
    type Target = RefCell<RingBuffer<E>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<E> QueueHandle<E> for RcStorageHandle<E> {
    type Storage = RefCell<RingBuffer<E>>;

    fn storage(&self) -> &Self::Storage {
      &self.0
    }
  }

  impl<E> crate::sync::Shared<RefCell<RingBuffer<E>>> for RcStorageHandle<E> {}

  struct RcBackendHandle<E>(Rc<RingStorageBackend<RcStorageHandle<E>>>);

  impl<E> Clone for RcBackendHandle<E> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<E> core::ops::Deref for RcBackendHandle<E> {
    type Target = RingStorageBackend<RcStorageHandle<E>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<E> crate::sync::Shared<RingStorageBackend<RcStorageHandle<E>>> for RcBackendHandle<E> {}

  impl<E> RingHandle<E> for RcBackendHandle<E> {
    type Backend = RingStorageBackend<RcStorageHandle<E>>;

    fn backend(&self) -> &Self::Backend {
      &self.0
    }
  }

  #[test]
  fn shared_ring_queue_offer_poll() {
    let storage = RcStorageHandle(Rc::new(RefCell::new(RingBuffer::new(2))));
    let backend = RcBackendHandle(Rc::new(RingStorageBackend::new(storage)));
    let queue = RingQueue::new(backend);

    queue.offer(1).unwrap();
    queue.offer(2).unwrap();
    assert_eq!(queue.poll().unwrap(), Some(1));
    assert_eq!(queue.poll().unwrap(), Some(2));
    assert_eq!(queue.poll().unwrap(), None);
  }
}
