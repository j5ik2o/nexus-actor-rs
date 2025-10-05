use core::ops::{Deref, DerefMut};

/// Abstracts over interior-mutable cells used to store actor state.
///
/// The trait is intentionally lightweight: runtimes can provide `Rc<RefCell<T>>`,
/// `Arc<Mutex<T>>` など環境に応じた実装を用意しつつ、同じ API を通じて
/// 状態を参照・更新できるようにする。
pub trait StateCell<T>: Clone {
  type Ref<'a>: Deref<Target = T>
  where
    Self: 'a,
    T: 'a;

  type RefMut<'a>: DerefMut<Target = T>
  where
    Self: 'a,
    T: 'a;

  /// Construct a new state cell with the provided value.
  fn new(value: T) -> Self
  where
    Self: Sized;

  /// Borrow the state immutably.
  fn borrow(&self) -> Self::Ref<'_>;

  /// Borrow the state mutably.
  fn borrow_mut(&self) -> Self::RefMut<'_>;

  /// Execute the provided closure with an immutable borrow of the state.
  fn with_ref<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    let guard = self.borrow();
    f(&*guard)
  }

  /// Execute the provided closure with a mutable borrow of the state.
  fn with_ref_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
    let mut guard = self.borrow_mut();
    f(&mut *guard)
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;

  use super::*;
  use alloc::rc::Rc;
  use core::cell::{Ref, RefCell, RefMut};

  struct RcState<T>(Rc<RefCell<T>>);

  impl<T> Clone for RcState<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> StateCell<T> for RcState<T> {
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

    fn new(value: T) -> Self {
      Self(Rc::new(RefCell::new(value)))
    }

    fn borrow(&self) -> Self::Ref<'_> {
      self.0.borrow()
    }

    fn borrow_mut(&self) -> Self::RefMut<'_> {
      self.0.borrow_mut()
    }
  }

  #[test]
  fn with_ref_reads_current_value() {
    let cell = RcState::new(5_u32);
    let value = cell.with_ref(|v| *v);
    assert_eq!(value, 5);
  }

  #[test]
  fn with_ref_mut_updates_value() {
    let cell = RcState::new(1_u32);
    cell.with_ref_mut(|v| *v = 10);
    assert_eq!(cell.with_ref(|v| *v), 10);
  }
}
