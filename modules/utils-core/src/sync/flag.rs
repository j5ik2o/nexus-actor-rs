use core::fmt::{self, Debug, Formatter};

#[cfg(feature = "alloc")]
use alloc::sync::Arc;

#[cfg(not(feature = "alloc"))]
use core::cell::Cell;
#[cfg(feature = "alloc")]
use core::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone)]
pub struct Flag {
  #[cfg(feature = "alloc")]
  inner: Arc<AtomicBool>,
  #[cfg(not(feature = "alloc"))]
  inner: Cell<bool>,
}

impl Flag {
  pub fn new(value: bool) -> Self {
    #[cfg(feature = "alloc")]
    {
      Self {
        inner: Arc::new(AtomicBool::new(value)),
      }
    }

    #[cfg(not(feature = "alloc"))]
    {
      Self {
        inner: Cell::new(value),
      }
    }
  }

  pub fn set(&self, value: bool) {
    #[cfg(feature = "alloc")]
    {
      self.inner.store(value, Ordering::SeqCst);
    }

    #[cfg(not(feature = "alloc"))]
    {
      self.inner.set(value);
    }
  }

  pub fn get(&self) -> bool {
    #[cfg(feature = "alloc")]
    {
      return self.inner.load(Ordering::SeqCst);
    }

    #[cfg(not(feature = "alloc"))]
    {
      return self.inner.get();
    }
  }

  pub fn clear(&self) {
    self.set(false);
  }
}

impl Default for Flag {
  fn default() -> Self {
    Self::new(false)
  }
}

impl Debug for Flag {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.debug_struct("Flag").field("value", &self.get()).finish()
  }
}
