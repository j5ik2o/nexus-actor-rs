use core::ops::Deref;

/// Shared ownership abstraction used across runtimes.
pub trait Shared<T: ?Sized>: Clone + Deref<Target = T> {
  /// Attempt to unwrap the shared value. Implementations may override this to
  /// provide specialised behaviour (e.g. `Arc::try_unwrap`).
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Err(self)
  }

  /// Execute the provided closure with a shared reference to the inner value.
  fn with_ref<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    f(self.deref())
  }
}

/// Marker trait that expresses the synchronisation guarantees required for shared closures.
///
/// * On targets that provide atomic pointer operations (`target_has_atomic = "ptr"`), this marker
///   requires `Send + Sync`, matching the capabilities of `alloc::sync::Arc`.
/// * On targets without atomic support (e.g. RP2040), the marker imposes no additional bounds so
///   that `Rc`-backed implementations can be used safely in single-threaded contexts.
#[cfg(target_has_atomic = "ptr")]
pub trait SharedBound: Send + Sync {}

#[cfg(target_has_atomic = "ptr")]
impl<T: Send + Sync> SharedBound for T {}

#[cfg(not(target_has_atomic = "ptr"))]
/// Marker trait used when atomic pointer support is unavailable.
pub trait SharedBound {}

#[cfg(not(target_has_atomic = "ptr"))]
impl<T> SharedBound for T {}

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
