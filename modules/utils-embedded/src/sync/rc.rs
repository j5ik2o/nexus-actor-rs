use alloc::rc::Rc;
use core::cell::{Ref, RefCell, RefMut};
use core::ops::Deref;

use nexus_utils_core_rs::{
  MpscBackend, MpscHandle, QueueHandle, QueueStorage, RingBackend, RingHandle, StackBackend, StackHandle,
};
use nexus_utils_core_rs::{Shared, StateCell};

/// `Rc` ベースの共有参照ラッパー。
///
/// `no_std` 環境で、`Rc` を使用した共有所有権を提供します。
/// `Shared` トレイトを実装し、各種コレクションバックエンドのハンドルとして使用できます。
///
/// # 特徴
///
/// - `Rc` による参照カウント（シングルスレッド専用）
/// - `Deref` による透過的なアクセス
/// - `try_unwrap` による所有権の取り戻し
#[derive(Debug)]
pub struct RcShared<T>(Rc<T>);

impl<T> RcShared<T> {
  /// 指定された値で新しい共有参照を作成します。
  pub fn new(value: T) -> Self {
    Self(Rc::new(value))
  }

  /// 既存の `Rc` から共有参照を作成します。
  pub fn from_rc(rc: Rc<T>) -> Self {
    Self(rc)
  }

  /// 内部の `Rc` を取り出します。
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

/// `Rc<RefCell<T>>` ベースの状態セル。
///
/// `no_std` 環境で、`Rc` と `RefCell` を使用した共有可変状態を提供します。
/// `StateCell` トレイトを実装し、内部可変性パターンを実現します。
///
/// # 特徴
///
/// - `Rc` による参照カウント（シングルスレッド専用）
/// - `RefCell` による内部可変性
/// - 実行時借用チェック
///
/// # 使用例
///
/// ```ignore
/// let cell = RcStateCell::new(1);
/// let cloned = cell.clone();
///
/// {
///   let mut value = cloned.borrow_mut();
///   *value = 5;
/// }
///
/// assert_eq!(*cell.borrow(), 5);
/// ```
#[derive(Debug)]
pub struct RcStateCell<T>(Rc<RefCell<T>>);

impl<T> RcStateCell<T> {
  /// 指定された値で新しい状態セルを作成します。
  pub fn new(value: T) -> Self {
    <Self as StateCell<T>>::new(value)
  }

  /// 既存の `Rc<RefCell<T>>` から状態セルを作成します。
  pub fn from_rc(rc: Rc<RefCell<T>>) -> Self {
    Self(rc)
  }

  /// 内部の `Rc<RefCell<T>>` を取り出します。
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
