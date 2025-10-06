use alloc::rc::Rc;
use core::cell::{Ref, RefCell, RefMut};
use core::ops::Deref;

use nexus_utils_core_rs::sync::{Shared, StateCell};
use nexus_utils_core_rs::{
  MpscBackend, MpscHandle, QueueHandle, QueueStorage, RingBackend, RingHandle, StackBackend, StackHandle,
};

#[derive(Debug)]
pub struct RcShared<T>(Rc<T>);

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

impl<T> Clone for RcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T> Deref for RcShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T> Shared<T> for RcShared<T> {
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Rc::try_unwrap(self.0).map_err(RcShared)
  }
}

impl<T, E> QueueHandle<E> for RcShared<T>
where
  T: QueueStorage<E>,
{
  type Storage = T;

  fn storage(&self) -> &Self::Storage {
    &self.0
  }
}

impl<T, B> MpscHandle<T> for RcShared<B>
where
  B: MpscBackend<T>,
{
  type Backend = B;

  fn backend(&self) -> &Self::Backend {
    &self.0
  }
}

impl<E, B> RingHandle<E> for RcShared<B>
where
  B: RingBackend<E>,
{
  type Backend = B;

  fn backend(&self) -> &Self::Backend {
    &self.0
  }
}

impl<T, B> StackHandle<T> for RcShared<B>
where
  B: StackBackend<T>,
{
  type Backend = B;

  fn backend(&self) -> &Self::Backend {
    &self.0
  }
}

#[derive(Debug)]
pub struct RcStateCell<T>(Rc<RefCell<T>>);

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

impl<T> Clone for RcStateCell<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rc_state_cell_updates() {
    let cell = RcStateCell::new(1_u32);
    let cloned = cell.clone();

    {
      let mut value = cloned.borrow_mut();
      *value = 5;
    }

    assert_eq!(*cell.borrow(), 5);
  }

  #[test]
  fn rc_shared_try_unwrap_behavior() {
    let shared = RcShared::new(10_u32);
    let clone = shared.clone();

    assert!(clone.try_unwrap().is_err());
    assert_eq!(RcShared::new(5_u32).try_unwrap().unwrap(), 5);
  }

  #[test]
  fn rc_shared_conversion_round_trip() {
    let rc = Rc::new(3_u32);
    let shared = RcShared::from_rc(rc.clone());
    assert!(Rc::ptr_eq(&shared.clone().into_inner(), &rc));
  }
}
