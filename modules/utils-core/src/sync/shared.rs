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
}
