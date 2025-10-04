use std::ops::Deref;
use std::sync::Arc;

use nexus_utils_core_rs::sync::Shared;

#[derive(Debug)]
pub struct ArcShared<T>(Arc<T>);

impl<T> ArcShared<T> {
  pub fn new(value: T) -> Self {
    Self(Arc::new(value))
  }

  pub fn from_arc(inner: Arc<T>) -> Self {
    Self(inner)
  }

  pub fn into_arc(self) -> Arc<T> {
    self.0
  }
}

impl<T> Deref for ArcShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T> Shared<T> for ArcShared<T> {
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Arc::try_unwrap(self.0).map_err(ArcShared)
  }
}

impl<T> Clone for ArcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}
