use alloc::sync::Arc;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use embassy_sync::mutex::{Mutex, MutexGuard};
use nexus_utils_core_rs::sync::{Shared, StateCell};
use nexus_utils_core_rs::{
  MpscBackend, MpscBuffer, QueueHandle, QueueStorage, RingBuffer, RingBufferBackend, RingBufferStorage,
  SharedMpscHandle,
};

pub struct ArcShared<T: ?Sized>(Arc<T>);

impl<T: ?Sized> core::fmt::Debug for ArcShared<T> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("ArcShared").finish()
  }
}

impl<T> ArcShared<T>
where
  T: Sized,
{
  pub fn new(value: T) -> Self {
    Self(Arc::new(value))
  }
}

impl<T: ?Sized> ArcShared<T> {
  pub fn from_arc(inner: Arc<T>) -> Self {
    Self(inner)
  }

  pub fn into_arc(self) -> Arc<T> {
    self.0
  }
}

impl<T: ?Sized> Clone for ArcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
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

impl<T, E> QueueHandle<E> for ArcShared<T>
where
  T: QueueStorage<E>,
{
  type Storage = T;

  fn storage(&self) -> &Self::Storage {
    &self.0
  }
}

impl<T, B> SharedMpscHandle<T> for ArcShared<B>
where
  B: MpscBackend<T>,
{
  type Backend = B;

  fn backend(&self) -> &Self::Backend {
    &self.0
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

impl<T, RM> RingBufferStorage<T> for ArcStateCell<MpscBuffer<T>, RM>
where
  RM: RawMutex,
{
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
    let guard = self.borrow();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
    let mut guard = self.borrow_mut();
    f(&mut guard)
  }
}

impl<E, RM> QueueStorage<E> for ArcStateCell<RingBuffer<E>, RM>
where
  RM: RawMutex,
{
  fn with_read<R>(&self, f: impl FnOnce(&RingBuffer<E>) -> R) -> R {
    let guard = self.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R {
    let mut guard = self.lock();
    f(&mut guard)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tests::init_arc_critical_section;

  fn prepare() {
    init_arc_critical_section();
  }

  #[test]
  fn arc_state_cell_updates() {
    prepare();
    let cell = ArcLocalStateCell::new(0_u32);
    let cloned = cell.clone();

    {
      let mut value = cloned.borrow_mut();
      *value = 9;
    }

    assert_eq!(*cell.borrow(), 9);
  }

  #[test]
  fn arc_shared_try_unwrap() {
    prepare();
    let shared = ArcShared::new(7_u32);
    assert_eq!(ArcShared::new(3_u32).try_unwrap().unwrap(), 3);
    let clone = shared.clone();
    assert!(clone.try_unwrap().is_err());
  }

  #[test]
  fn arc_state_cell_into_arc_exposes_inner() {
    prepare();
    let cell = ArcLocalStateCell::new(5_u32);
    let arc = cell.clone().into_arc();
    assert_eq!(*arc.try_lock().unwrap(), 5);
  }
}
