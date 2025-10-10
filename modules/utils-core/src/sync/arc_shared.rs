#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as Arc;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc;

use super::Shared;

/// Shared wrapper backed by `alloc::sync::Arc`.
///
/// Targets that lack atomic pointer operations (`target_has_atomic = "ptr"`)
/// do not provide `alloc::sync::Arc`. In those environments we transparently
/// fall back to `alloc::rc::Rc`, allowing higher layers to keep using a unified
/// shared abstraction.
pub struct ArcShared<T: ?Sized>(Arc<T>);

impl<T: ?Sized> ArcShared<T> {
  /// Creates a new `ArcShared` by wrapping the provided value.
  pub fn new(value: T) -> Self
  where
    T: Sized, {
    Self(Arc::new(value))
  }

  /// Wraps an existing `Arc` in the shared wrapper.
  pub fn from_arc(inner: Arc<T>) -> Self {
    Self(inner)
  }

  /// Consumes the wrapper and returns the inner `Arc`.
  pub fn into_arc(self) -> Arc<T> {
    self.0
  }
}

impl<T: ?Sized> core::ops::Deref for ArcShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T: ?Sized> Shared<T> for ArcShared<T> {
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Arc::try_unwrap(self.0).map_err(ArcShared)
  }
}

impl<T: ?Sized> Clone for ArcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}
