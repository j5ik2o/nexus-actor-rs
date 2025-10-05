use super::buffer::MpscBuffer;
use super::traits::MpscBackend;
use crate::collections::{QueueError, QueueSize};

/// Storage abstraction used by the ring-buffer based [`MpscBackend`].
pub trait RingBufferStorage<T> {
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R;
  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R;
}

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

#[cfg(feature = "alloc")]
mod alloc_impls {
  use core::cell::RefCell;

  use super::{MpscBuffer, RingBufferStorage};

  impl<T> RingBufferStorage<T> for RefCell<MpscBuffer<T>> {
    fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
      let guard = self.borrow();
      f(&guard)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
      let mut guard = self.borrow_mut();
      f(&mut guard)
    }
  }
}

#[cfg(all(feature = "alloc", feature = "std"))]
mod std_impls {
  use std::sync::Mutex;

  use super::{MpscBuffer, RingBufferStorage};

  impl<T> RingBufferStorage<T> for Mutex<MpscBuffer<T>> {
    fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
      let guard = self.lock().expect("mutex poisoned");
      f(&guard)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
      let mut guard = self.lock().expect("mutex poisoned");
      f(&mut guard)
    }
  }
}
