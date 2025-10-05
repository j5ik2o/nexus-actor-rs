use super::buffer::MpscBuffer;

/// Storage abstraction used by the ring-buffer based [`MpscBackend`].
pub trait RingBufferStorage<T> {
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R;
  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R;
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
