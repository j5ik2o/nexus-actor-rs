use core::ops::Deref;
use nexus_actor_core_rs::Shared;

#[cfg(feature = "embedded_rc")]
use alloc::rc::Rc;

#[cfg(feature = "embedded_rc")]
pub struct RcShared<T>(Rc<T>);

#[cfg(feature = "embedded_rc")]
impl<T> RcShared<T> {
  pub fn new(value: T) -> Self {
    Self(Rc::new(value))
  }

  pub fn from_rc(rc: Rc<T>) -> Self {
    Self(rc)
  }

  pub fn into_inner(self) -> Rc<T> {
    self.0
  }
}

#[cfg(feature = "embedded_rc")]
impl<T> Clone for RcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

#[cfg(feature = "embedded_rc")]
impl<T> Deref for RcShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[cfg(feature = "embedded_rc")]
impl<T> Shared<T> for RcShared<T> {
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Rc::try_unwrap(self.0).map_err(RcShared)
  }
}
