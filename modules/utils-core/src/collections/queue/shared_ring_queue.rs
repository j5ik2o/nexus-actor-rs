use crate::sync::Shared;

use super::{QueueBase, QueueError, QueueReader, QueueSize, QueueWriter, RingBuffer, SharedQueue};

/// Abstraction over the mutable container that stores a [`RingBuffer`].
///
/// Each environment provides an implementation (e.g. `RefCell`, `Mutex`) that
/// exposes read/write closures while hiding the synchronization mechanism.
pub trait QueueStorage<E> {
  /// Execute the provided closure with a shared reference to the underlying
  /// [`RingBuffer`].
  fn with_read<R>(&self, f: impl FnOnce(&RingBuffer<E>) -> R) -> R;

  /// Execute the provided closure with a mutable reference to the underlying
  /// [`RingBuffer`].
  fn with_write<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R;
}

/// Shared pointer that grants access to a [`QueueStorage`] implementation.
///
/// The pointer itself must implement [`Shared`] so that cloning keeps a shared
/// view on the same storage instance irrespective of the synchronization
/// primitive used underneath.
pub trait SharedQueueHandle<E>: Shared<Self::Storage> + Clone {
  type Storage: QueueStorage<E> + ?Sized;

  fn storage(&self) -> &Self::Storage;
}

/// Queue facade built around a [`SharedQueueHandle`].
///
/// The structure forwards all queue operations to the `QueueStorage`
/// implementation exposed via the shared handle, ensuring that runtime-specific
/// synchronization remains outside the common logic layer.
#[derive(Debug, Clone)]
pub struct SharedRingQueue<S, E>
where
  S: SharedQueueHandle<E>, {
  storage: S,
  _marker: core::marker::PhantomData<E>,
}

impl<S, E> SharedRingQueue<S, E>
where
  S: SharedQueueHandle<E>,
{
  /// Construct a queue from the given shared storage handle.
  pub fn new(storage: S) -> Self {
    Self {
      storage,
      _marker: core::marker::PhantomData,
    }
  }

  /// Expose the underlying shared storage handle.
  pub fn storage(&self) -> &S {
    &self.storage
  }

  /// Consume the queue and return the storage handle.
  pub fn into_storage(self) -> S {
    self.storage
  }

  /// Enable or disable dynamic growth.
  pub fn set_dynamic(&self, dynamic: bool) {
    self.handle().with_write(|buffer| buffer.set_dynamic(dynamic));
  }

  /// Builder-style variant of [`set_dynamic`].
  pub fn with_dynamic(self, dynamic: bool) -> Self {
    self.set_dynamic(dynamic);
    self
  }

  /// Offer an element using the shared storage handle.
  pub fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    self.handle().with_write(|buffer| buffer.offer(element))
  }

  /// Poll an element using the shared storage handle.
  pub fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    self.handle().with_write(|buffer| buffer.poll())
  }

  /// Clean up remaining elements.
  pub fn clean_up_shared(&self) {
    self.handle().with_write(|buffer| buffer.clean_up());
  }

  /// Return the length as observed through the shared storage.
  pub fn len_shared(&self) -> QueueSize {
    self.handle().with_read(|buffer| buffer.len())
  }

  /// Return the capacity as observed through the shared storage.
  pub fn capacity_shared(&self) -> QueueSize {
    self.handle().with_read(|buffer| buffer.capacity())
  }

  fn handle(&self) -> &S::Storage {
    self.storage.storage()
  }
}

impl<S, E> QueueBase<E> for SharedRingQueue<S, E>
where
  S: SharedQueueHandle<E>,
{
  fn len(&self) -> QueueSize {
    self.len_shared()
  }

  fn capacity(&self) -> QueueSize {
    self.capacity_shared()
  }
}

impl<S, E> QueueWriter<E> for SharedRingQueue<S, E>
where
  S: SharedQueueHandle<E>,
{
  fn offer(&mut self, element: E) -> Result<(), QueueError<E>> {
    self.offer_shared(element)
  }
}

impl<S, E> QueueReader<E> for SharedRingQueue<S, E>
where
  S: SharedQueueHandle<E>,
{
  fn poll(&mut self) -> Result<Option<E>, QueueError<E>> {
    self.poll_shared()
  }

  fn clean_up(&mut self) {
    self.clean_up_shared();
  }
}

impl<S, E> SharedQueue<E> for SharedRingQueue<S, E>
where
  S: SharedQueueHandle<E>,
{
  fn offer_shared(&self, element: E) -> Result<(), QueueError<E>> {
    SharedRingQueue::offer_shared(self, element)
  }

  fn poll_shared(&self) -> Result<Option<E>, QueueError<E>> {
    SharedRingQueue::poll_shared(self)
  }

  fn clean_up_shared(&self) {
    SharedRingQueue::clean_up_shared(self)
  }
}

#[cfg(feature = "alloc")]
mod alloc_impls {
  use core::cell::RefCell;

  use super::{QueueStorage, RingBuffer};

  impl<E> QueueStorage<E> for RefCell<RingBuffer<E>> {
    fn with_read<R>(&self, f: impl FnOnce(&RingBuffer<E>) -> R) -> R {
      let guard = self.borrow();
      f(&guard)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R {
      let mut guard = self.borrow_mut();
      f(&mut guard)
    }
  }
}

#[cfg(all(feature = "alloc", feature = "std"))]
mod std_impls {
  use std::sync::Mutex;

  use super::{QueueStorage, RingBuffer};

  impl<E> QueueStorage<E> for Mutex<RingBuffer<E>> {
    fn with_read<R>(&self, f: impl FnOnce(&RingBuffer<E>) -> R) -> R {
      let guard = self.lock().expect("mutex poisoned");
      f(&guard)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R {
      let mut guard = self.lock().expect("mutex poisoned");
      f(&mut guard)
    }
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;

  use alloc::rc::Rc;
  use core::cell::RefCell;
  use core::ops::Deref;

  use crate::sync::Shared;

  use super::{QueueError, QueueSize, RingBuffer, SharedQueueHandle, SharedRingQueue};

  #[derive(Debug)]
  struct RcRingBufferHandle<E>(Rc<RefCell<RingBuffer<E>>>);

  impl<E> RcRingBufferHandle<E> {
    fn new(capacity: usize) -> Self {
      Self(Rc::new(RefCell::new(RingBuffer::new(capacity))))
    }
  }

  impl<E> Clone for RcRingBufferHandle<E> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<E> Deref for RcRingBufferHandle<E> {
    type Target = RefCell<RingBuffer<E>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<E> Shared<RefCell<RingBuffer<E>>> for RcRingBufferHandle<E> {}

  impl<E> SharedQueueHandle<E> for RcRingBufferHandle<E> {
    type Storage = RefCell<RingBuffer<E>>;

    fn storage(&self) -> &Self::Storage {
      &self.0
    }
  }

  #[test]
  fn shared_ring_queue_offer_poll() {
    let queue: SharedRingQueue<_, _> = SharedRingQueue::new(RcRingBufferHandle::new(2)).with_dynamic(false);
    assert!(queue.offer_shared(1).is_ok());
    assert!(queue.offer_shared(2).is_ok());
    assert_eq!(queue.offer_shared(3), Err(QueueError::Full(3)));

    assert_eq!(queue.poll_shared().unwrap(), Some(1));
    assert_eq!(queue.poll_shared().unwrap(), Some(2));
    assert_eq!(queue.poll_shared().unwrap(), None);
  }

  #[test]
  fn shared_ring_queue_len_capacity() {
    let queue: SharedRingQueue<_, _> = SharedRingQueue::new(RcRingBufferHandle::new(4)).with_dynamic(false);
    assert_eq!(queue.len_shared(), QueueSize::limited(0));
    assert_eq!(queue.capacity_shared(), QueueSize::limited(4));

    queue.offer_shared(10).unwrap();
    assert_eq!(queue.len_shared(), QueueSize::limited(1));
  }
}
