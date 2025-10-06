use super::buffer::RingBuffer;

pub trait QueueStorage<E> {
  fn with_read<R>(&self, f: impl FnOnce(&RingBuffer<E>) -> R) -> R;
  fn with_write<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R;
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
