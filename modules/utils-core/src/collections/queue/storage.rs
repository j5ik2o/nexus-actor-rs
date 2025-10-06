use super::{mpsc::MpscBuffer, ring::RingBuffer};

pub trait QueueStorage<E> {
  fn with_read<R>(&self, f: impl FnOnce(&RingBuffer<E>) -> R) -> R;
  fn with_write<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R;
}

/// Storage abstraction shared by ring-buffer based [`crate::collections::queue::mpsc::RingBufferBackend`]
/// implementations.
pub trait RingBufferStorage<T> {
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R;
  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R;
}

#[cfg(feature = "alloc")]
mod queue_alloc_impls {
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
mod queue_std_impls {
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

#[cfg(feature = "alloc")]
mod mpsc_alloc_impls {
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
mod mpsc_std_impls {
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
