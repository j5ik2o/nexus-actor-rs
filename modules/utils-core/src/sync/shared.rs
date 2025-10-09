use core::ops::Deref;

/// Shared ownership abstraction used across runtimes.
pub trait Shared<T: ?Sized>: Clone + Deref<Target = T> {
  /// Attempt to unwrap the shared value. Implementations may override this to
  /// provide specialised behaviour (e.g. `Arc::try_unwrap`).
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized,
  {
    Err(self)
  }

  /// Execute the provided closure with a shared reference to the inner value.
  fn with_ref<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    f(self.deref())
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;

  use super::*;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  #[derive(Clone, Debug)]
  struct RcSharedCell(Rc<RefCell<u32>>);

  impl RcSharedCell {
    fn new(value: u32) -> Self {
      Self(Rc::new(RefCell::new(value)))
    }
  }

  impl Deref for RcSharedCell {
    type Target = RefCell<u32>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl Shared<RefCell<u32>> for RcSharedCell {}

  #[test]
  fn default_try_unwrap_returns_err() {
    let shared = RcSharedCell::new(10);
    let result = shared.clone().try_unwrap();
    assert!(result.is_err(), "default try_unwrap should return Err");
  }

  #[test]
  fn with_ref_exposes_inner_value() {
    let shared = RcSharedCell::new(7);
    let value = shared.with_ref(|cell| *cell.borrow());
    assert_eq!(value, 7);
  }
}
