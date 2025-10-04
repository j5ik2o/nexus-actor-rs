use std::sync::{Arc, Mutex, MutexGuard};

use nexus_actor_core_rs::StateCell;

/// Arc-based state cell backed by `std::sync::Mutex` for synchronous access on std runtimes.
pub struct ArcStateCell<T>(Arc<Mutex<T>>);

impl<T> ArcStateCell<T> {
  pub fn new(value: T) -> Self {
    Self(Arc::new(Mutex::new(value)))
  }

  pub fn from_arc(inner: Arc<Mutex<T>>) -> Self {
    Self(inner)
  }

  pub fn into_arc(self) -> Arc<Mutex<T>> {
    self.0
  }
}

impl<T> Clone for ArcStateCell<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T> StateCell<T> for ArcStateCell<T> {
  type Ref<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a,
    T: 'a;
  type RefMut<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self
  where
    Self: Sized, {
    ArcStateCell::new(value)
  }

  fn borrow(&self) -> Self::Ref<'_> {
    self.0.lock().expect("mutex poisoned")
  }

  fn borrow_mut(&self) -> Self::RefMut<'_> {
    self.0.lock().expect("mutex poisoned")
  }
}
