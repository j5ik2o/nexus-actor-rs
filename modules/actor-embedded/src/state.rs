use nexus_actor_core_rs::StateCell;

#[cfg(feature = "embedded_rc")]
use alloc::rc::Rc;
#[cfg(feature = "embedded_rc")]
use core::cell::{Ref, RefCell, RefMut};

#[cfg(feature = "embedded_arc")]
use alloc::sync::Arc;
#[cfg(feature = "embedded_arc")]
use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
#[cfg(feature = "embedded_arc")]
use embassy_sync::mutex::{Mutex, MutexGuard};

#[cfg(feature = "embedded_rc")]
pub struct RcStateCell<T>(Rc<RefCell<T>>);

#[cfg(feature = "embedded_rc")]
impl<T> RcStateCell<T> {
  pub fn new(value: T) -> Self {
    <Self as StateCell<T>>::new(value)
  }

  pub fn from_rc(rc: Rc<RefCell<T>>) -> Self {
    Self(rc)
  }

  pub fn into_rc(self) -> Rc<RefCell<T>> {
    self.0
  }
}

#[cfg(feature = "embedded_rc")]
impl<T> Clone for RcStateCell<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

#[cfg(feature = "embedded_rc")]
impl<T> StateCell<T> for RcStateCell<T> {
  type Ref<'a>
    = Ref<'a, T>
  where
    Self: 'a,
    T: 'a;
  type RefMut<'a>
    = RefMut<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self
  where
    Self: Sized, {
    Self(Rc::new(RefCell::new(value)))
  }

  fn borrow(&self) -> Self::Ref<'_> {
    self.0.borrow()
  }

  fn borrow_mut(&self) -> Self::RefMut<'_> {
    self.0.borrow_mut()
  }
}

#[cfg(feature = "embedded_arc")]
pub struct ArcStateCell<T, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: Arc<Mutex<RM, T>>,
}

#[cfg(feature = "embedded_arc")]
pub type ArcCsStateCell<T> = ArcStateCell<T, CriticalSectionRawMutex>;
#[cfg(feature = "embedded_arc")]
pub type ArcLocalStateCell<T> = ArcStateCell<T, NoopRawMutex>;

#[cfg(feature = "embedded_arc")]
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

#[cfg(feature = "embedded_arc")]
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

#[cfg(feature = "embedded_arc")]
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
  extern crate std;

  use super::*;

  #[cfg(feature = "embedded_rc")]
  #[test]
  fn rc_state_cell_borrow_mut_applies_changes() {
    let cell = RcStateCell::new(0_u32);

    {
      let mut value = cell.borrow_mut();
      *value = 42;
    }

    assert_eq!(*cell.borrow(), 42);
  }

  #[cfg(feature = "embedded_rc")]
  #[test]
  fn rc_state_cell_clone_shares_state() {
    let cell = RcStateCell::new(10_u32);
    let cloned = cell.clone();

    {
      let mut value = cloned.borrow_mut();
      *value += 5;
    }

    assert_eq!(*cell.borrow(), 15);
  }

  #[cfg(feature = "embedded_arc")]
  #[test]
  fn arc_state_cell_borrow_updates_shared_value() {
    let cell = ArcLocalStateCell::new(1_u32);
    let cloned = cell.clone();

    {
      let mut value = cloned.borrow_mut();
      *value = 7;
    }

    assert_eq!(*cell.borrow(), 7);
  }
}
