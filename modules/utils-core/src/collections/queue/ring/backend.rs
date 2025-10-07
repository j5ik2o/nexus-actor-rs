use crate::collections::queue::traits::{QueueBase, QueueReader, QueueWriter};
use crate::collections::queue::QueueStorage;
use crate::collections::{QueueError, QueueSize};
use crate::sync::Shared;

/// Backend abstraction for ring-buffer based queues.
pub trait RingBackend<E> {
  fn offer(&self, element: E) -> Result<(), QueueError<E>>;
  fn poll(&self) -> Result<Option<E>, QueueError<E>>;
  fn clean_up(&self);
  fn len(&self) -> QueueSize;
  fn capacity(&self) -> QueueSize;
  fn set_dynamic(&self, dynamic: bool);

  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }
}

/// Handle that dereferences to a [`RingBackend`].
pub trait RingHandle<E>: Shared<Self::Backend> + Clone {
  type Backend: RingBackend<E> + ?Sized;

  fn backend(&self) -> &Self::Backend;
}

/// Backend implementation that operates directly on a [`RingBuffer`] storage handle.
#[derive(Debug)]
pub struct RingStorageBackend<S> {
  storage: S,
}

impl<S> RingStorageBackend<S> {
  pub const fn new(storage: S) -> Self {
    Self { storage }
  }

  pub fn storage(&self) -> &S {
    &self.storage
  }

  pub fn into_storage(self) -> S {
    self.storage
  }
}

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
