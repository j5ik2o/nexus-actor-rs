use crate::collections::mpsc::buffer::MpscBuffer;
use crate::sync::Shared;

/// Storage abstraction for [`SharedMpscQueue`].
pub trait MpscStorage<T> {
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R;
  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R;
}

/// Shared handle that exposes the underlying storage.
pub trait SharedMpscHandle<T>: Shared<Self::Storage> + Clone {
  type Storage: MpscStorage<T> + ?Sized;

  fn storage(&self) -> &Self::Storage;
}

#[cfg(feature = "alloc")]
mod alloc_impls {
  use core::cell::RefCell;

  use super::{MpscBuffer, MpscStorage};

  impl<T> MpscStorage<T> for RefCell<MpscBuffer<T>> {
    fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
      f(&self.borrow())
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
      f(&mut self.borrow_mut())
    }
  }
}

#[cfg(all(feature = "alloc", feature = "std"))]
mod std_impls {
  use std::sync::Mutex;

  use super::{MpscBuffer, MpscStorage};

  impl<T> MpscStorage<T> for Mutex<MpscBuffer<T>> {
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
