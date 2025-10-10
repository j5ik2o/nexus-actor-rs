#[cfg(feature = "alloc")]
use alloc::rc::Rc;
#[cfg(feature = "alloc")]
use core::ops::Deref;

use super::Shared;

/// Shared ownership wrapper backed by `alloc::rc::Rc`.
///
/// Unlike [`ArcShared`](super::ArcShared), this variant deliberately omits any
/// thread-safety guarantees, making it suitable for single-threaded runtimes or
/// bare-metal targets where atomic pointer operations are unavailable.
#[cfg(feature = "alloc")]
pub struct RcShared<T: ?Sized>(Rc<T>);

#[cfg(feature = "alloc")]
impl<T> RcShared<T> {
  /// Creates a new `RcShared` by wrapping the provided value.
  pub fn new(value: T) -> Self {
    Self(Rc::new(value))
  }
}

#[cfg(feature = "alloc")]
impl<T: ?Sized> RcShared<T> {
  /// Wraps an existing `Rc` in the shared wrapper.
  pub fn from_rc(inner: Rc<T>) -> Self {
    Self(inner)
  }

  /// Consumes the wrapper and returns the inner `Rc`.
  pub fn into_rc(self) -> Rc<T> {
    self.0
  }
}

#[cfg(feature = "alloc")]
impl<T: ?Sized> Clone for RcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

#[cfg(feature = "alloc")]
impl<T: ?Sized> Deref for RcShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[cfg(feature = "alloc")]
impl<T: ?Sized> Shared<T> for RcShared<T> {
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Rc::try_unwrap(self.0).map_err(RcShared)
  }
}
