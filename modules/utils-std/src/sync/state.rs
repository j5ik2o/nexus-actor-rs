use std::sync::{Arc, Mutex, MutexGuard};

use nexus_utils_core_rs::StateCell;

/// `Arc`と`Mutex`による共有可変状態セル
///
/// 複数のスレッド間で安全に共有可能な可変状態を提供します。
/// `StateCell`トレイトを実装しており、一貫したAPIで状態にアクセスできます。
#[derive(Debug)]
pub struct ArcStateCell<T>(Arc<Mutex<T>>);

impl<T> ArcStateCell<T> {
  /// 値から新しい`ArcStateCell`を作成します
  ///
  /// # Arguments
  ///
  /// * `value` - 初期値
  ///
  /// # Returns
  ///
  /// 新しい`ArcStateCell`インスタンス
  pub fn new(value: T) -> Self {
    Self(Arc::new(Mutex::new(value)))
  }

  /// 既存の`Arc<Mutex<T>>`から`ArcStateCell`を作成します
  ///
  /// # Arguments
  ///
  /// * `inner` - `Arc<Mutex<T>>`インスタンス
  ///
  /// # Returns
  ///
  /// `Arc<Mutex<T>>`をラップした`ArcStateCell`インスタンス
  pub fn from_arc(inner: Arc<Mutex<T>>) -> Self {
    Self(inner)
  }

  /// `ArcStateCell`を内部の`Arc<Mutex<T>>`に変換します
  ///
  /// # Returns
  ///
  /// 内部の`Arc<Mutex<T>>`インスタンス
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
