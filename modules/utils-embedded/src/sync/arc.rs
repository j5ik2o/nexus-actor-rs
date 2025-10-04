use alloc::sync::Arc;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use embassy_sync::mutex::{Mutex, MutexGuard};
use nexus_utils_core_rs::sync::{Shared, StateCell};

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

impl<T> Clone for ArcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T> core::ops::Deref for ArcShared<T> {
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

#[derive(Debug)]
pub struct ArcStateCell<T, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: Arc<Mutex<RM, T>>,
}

pub type ArcLocalStateCell<T> = ArcStateCell<T, NoopRawMutex>;
pub type ArcCsStateCell<T> = ArcStateCell<T, CriticalSectionRawMutex>;

impl<T, RM> ArcStateCell<T, RM>
where
  RM: RawMutex,
{
  pub fn new(value: T) -> Self {
    Self {
      inner: Arc::new(Mutex::new(value)),
    }
  }

  pub fn from_arc(inner: Arc<Mutex<RM, T>>) -> Self {
    Self { inner }
  }

  pub fn into_arc(self) -> Arc<Mutex<RM, T>> {
    self.inner
  }

  fn lock(&self) -> MutexGuard<'_, RM, T> {
    self
      .inner
      .try_lock()
      .unwrap_or_else(|_| panic!("ArcStateCell: concurrent access detected"))
  }
}

impl<T, RM> Clone for ArcStateCell<T, RM>
where
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

impl<T, RM> StateCell<T> for ArcStateCell<T, RM>
where
  RM: RawMutex,
{
  type Ref<'a>
    = MutexGuard<'a, RM, T>
  where
    Self: 'a,
    T: 'a;
  type RefMut<'a>
    = MutexGuard<'a, RM, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self
  where
    Self: Sized, {
    ArcStateCell::new(value)
  }

  fn borrow(&self) -> Self::Ref<'_> {
    self.lock()
  }

  fn borrow_mut(&self) -> Self::RefMut<'_> {
    self.lock()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn arc_state_cell_updates() {
    let cell = ArcLocalStateCell::new(0_u32);
    let cloned = cell.clone();

    {
      let mut value = cloned.borrow_mut();
      *value = 9;
    }

    assert_eq!(*cell.borrow(), 9);
  }
}
