use std::sync::{Arc, Mutex, MutexGuard};

use nexus_utils_core_rs::StateCell;

/// Shared mutable state cell using `Arc` and `Mutex`
///
/// Provides mutable state that can be safely shared across multiple threads.
/// Implements the `StateCell` trait, enabling consistent API access to the state.
#[derive(Debug)]
pub struct ArcStateCell<T>(Arc<Mutex<T>>);

impl<T> ArcStateCell<T> {
  /// Creates a new `ArcStateCell` from a value
  ///
  /// # Arguments
  ///
  /// * `value` - The initial value
  ///
  /// # Returns
  ///
  /// A new `ArcStateCell` instance
  pub fn new(value: T) -> Self {
    Self(Arc::new(Mutex::new(value)))
  }

  /// Creates `ArcStateCell` from an existing `Arc<Mutex<T>>`
  ///
  /// # Arguments
  ///
  /// * `inner` - An `Arc<Mutex<T>>` instance
  ///
  /// # Returns
  ///
  /// An `ArcStateCell` instance wrapping the `Arc<Mutex<T>>`
  pub fn from_arc(inner: Arc<Mutex<T>>) -> Self {
    Self(inner)
  }

  /// Converts `ArcStateCell` to the internal `Arc<Mutex<T>>`
  ///
  /// # Returns
  ///
  /// The internal `Arc<Mutex<T>>` instance
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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn arc_state_cell_updates() {
    let cell = ArcStateCell::new(0_u32);
    let cloned = cell.clone();

    {
      let mut value = cloned.borrow_mut();
      *value = 5;
    }

    assert_eq!(*cell.borrow(), 5);
  }
}
