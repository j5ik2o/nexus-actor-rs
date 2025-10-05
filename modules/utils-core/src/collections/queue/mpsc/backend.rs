use super::storage::RingBufferStorage;
use super::traits::MpscBackend;
use crate::collections::{QueueError, QueueSize};

/// Backend that drives the shared multi-producer/single-consumer queue using an
/// in-memory [`MpscBuffer`].
#[derive(Debug)]
pub struct RingBufferBackend<S> {
  storage: S,
}

impl<S> RingBufferBackend<S> {
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

impl<S, T> MpscBackend<T> for RingBufferBackend<S>
where
  S: RingBufferStorage<T>,
{
  fn try_send(&self, element: T) -> Result<(), QueueError<T>> {
    self.storage.with_write(|buffer| buffer.offer(element))
  }

  fn try_recv(&self) -> Result<Option<T>, QueueError<T>> {
    self.storage.with_write(|buffer| buffer.poll())
  }

  fn close(&self) {
    self.storage.with_write(|buffer| buffer.clean_up());
  }

  fn len(&self) -> QueueSize {
    self.storage.with_read(|buffer| buffer.len())
  }

  fn capacity(&self) -> QueueSize {
    self.storage.with_read(|buffer| buffer.capacity())
  }

  fn is_closed(&self) -> bool {
    self.storage.with_read(|buffer| buffer.is_closed())
  }

  fn set_capacity(&self, capacity: Option<usize>) -> bool {
    self.storage.with_write(|buffer| buffer.set_capacity(capacity));
    true
  }
}
