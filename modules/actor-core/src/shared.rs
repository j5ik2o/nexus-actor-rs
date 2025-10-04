use core::ops::Deref;

/// Shared ownership abstraction used across runtimes.
pub trait Shared<T>: Clone + Deref<Target = T> {
  /// Attempt to unwrap the shared value. Implementations may override this to
  /// provide specialised behaviour (e.g. `Arc::try_unwrap`).
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Err(self)
  }
}

#[cfg(feature = "alloc")]
mod std_impl {
  use super::Shared;
  use alloc::sync::Arc;

  impl<T> Shared<T> for Arc<T> {
    fn try_unwrap(self) -> Result<T, Self>
    where
      T: Sized, {
      Arc::try_unwrap(self).map_err(|arc| arc)
    }
  }
}
