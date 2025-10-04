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
}
