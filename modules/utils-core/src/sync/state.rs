use core::ops::{Deref, DerefMut};

/// アクター状態を格納する内部可変セルの抽象化トレイト。
///
/// このトレイトは意図的に軽量に設計されており、ランタイムが環境に応じて
/// `Rc<RefCell<T>>`、`Arc<Mutex<T>>`、`Arc<RwLock<T>>` などの実装を提供しつつ、
/// 統一されたAPIを通じて状態を参照・更新できるようにします。
///
/// # 設計思想
///
/// - **抽象化**: 内部実装の詳細を隠蔽し、異なるランタイム環境で同じコードを使用可能に
/// - **柔軟性**: シングルスレッド環境では`Rc<RefCell<T>>`、マルチスレッド環境では`Arc<Mutex<T>>`など、適切な実装を選択可能
/// - **型安全性**: Generic Associated Types (GAT)を活用し、コンパイル時の型安全性を保証
///
/// # 実装例
///
/// ```rust
/// use std::rc::Rc;
/// use std::cell::{RefCell, Ref, RefMut};
/// # use core::ops::{Deref, DerefMut};
/// # pub trait StateCell<T>: Clone {
/// #   type Ref<'a>: Deref<Target = T> where Self: 'a, T: 'a;
/// #   type RefMut<'a>: DerefMut<Target = T> where Self: 'a, T: 'a;
/// #   fn new(value: T) -> Self where Self: Sized;
/// #   fn borrow(&self) -> Self::Ref<'_>;
/// #   fn borrow_mut(&self) -> Self::RefMut<'_>;
/// # }
///
/// // シングルスレッド環境向けの実装
/// struct RcState<T>(Rc<RefCell<T>>);
///
/// impl<T> Clone for RcState<T> {
///     fn clone(&self) -> Self {
///         Self(self.0.clone())
///     }
/// }
///
/// impl<T> StateCell<T> for RcState<T> {
///     type Ref<'a> = Ref<'a, T> where Self: 'a, T: 'a;
///     type RefMut<'a> = RefMut<'a, T> where Self: 'a, T: 'a;
///
///     fn new(value: T) -> Self {
///         Self(Rc::new(RefCell::new(value)))
///     }
///
///     fn borrow(&self) -> Self::Ref<'_> {
///         self.0.borrow()
///     }
///
///     fn borrow_mut(&self) -> Self::RefMut<'_> {
///         self.0.borrow_mut()
///     }
/// }
/// ```
pub trait StateCell<T>: Clone {
  /// 不変参照のガード型。
  ///
  /// `Deref<Target = T>`を実装し、スコープを抜けると自動的にロックが解放される
  /// RAII型として機能します。ランタイムの実装により、`Ref<'a, T>`、
  /// `MutexGuard<'a, T>`、`RwLockReadGuard<'a, T>`など異なる型が使用されます。
  type Ref<'a>: Deref<Target = T>
  where
    Self: 'a,
    T: 'a;

  /// 可変参照のガード型。
  ///
  /// `DerefMut<Target = T>`を実装し、スコープを抜けると自動的にロックが解放される
  /// RAII型として機能します。ランタイムの実装により、`RefMut<'a, T>`、
  /// `MutexGuard<'a, T>`、`RwLockWriteGuard<'a, T>`など異なる型が使用されます。
  type RefMut<'a>: DerefMut<Target = T>
  where
    Self: 'a,
    T: 'a;

  /// 指定された値で新しい状態セルを構築します。
  ///
  /// # 引数
  ///
  /// * `value` - 初期状態として格納する値
  ///
  /// # 戻り値
  ///
  /// 新しく作成された状態セルのインスタンス
  ///
  /// # 例
  ///
  /// ```rust,ignore
  /// let cell = RcState::new(42);
  /// ```
  fn new(value: T) -> Self
  where
    Self: Sized;

  /// 状態を不変的に借用します。
  ///
  /// このメソッドは、内部状態への読み取り専用アクセスを提供する
  /// ガード型を返します。ガードがスコープを抜けると、自動的にロックが解放されます。
  ///
  /// # 戻り値
  ///
  /// 状態への不変参照を保持するガードオブジェクト
  ///
  /// # パニック
  ///
  /// 実装によっては、既に可変借用が存在する場合にパニックする可能性があります
  /// （例: `RefCell`ベースの実装）。
  ///
  /// # 例
  ///
  /// ```rust,ignore
  /// let cell = RcState::new(42);
  /// let guard = cell.borrow();
  /// println!("Value: {}", *guard);
  /// ```
  fn borrow(&self) -> Self::Ref<'_>;

  /// 状態を可変的に借用します。
  ///
  /// このメソッドは、内部状態への読み書きアクセスを提供する
  /// ガード型を返します。ガードがスコープを抜けると、自動的にロックが解放されます。
  ///
  /// # 戻り値
  ///
  /// 状態への可変参照を保持するガードオブジェクト
  ///
  /// # パニック
  ///
  /// 実装によっては、既に借用が存在する場合にパニックする可能性があります
  /// （例: `RefCell`ベースの実装）。
  ///
  /// # 例
  ///
  /// ```rust,ignore
  /// let cell = RcState::new(42);
  /// let mut guard = cell.borrow_mut();
  /// *guard = 100;
  /// ```
  fn borrow_mut(&self) -> Self::RefMut<'_>;

  /// 状態への不変参照を使用してクロージャを実行します。
  ///
  /// このメソッドは、状態を借用し、その参照をクロージャに渡して実行します。
  /// クロージャの実行が完了すると、自動的にロックが解放されます。
  /// 手動でガードを管理する必要がないため、より安全で簡潔なコードが書けます。
  ///
  /// # 引数
  ///
  /// * `f` - 状態への不変参照を受け取り、任意の型`R`を返すクロージャ
  ///
  /// # 戻り値
  ///
  /// クロージャの実行結果
  ///
  /// # 例
  ///
  /// ```rust,ignore
  /// let cell = RcState::new(vec![1, 2, 3]);
  /// let len = cell.with_ref(|v| v.len());
  /// assert_eq!(len, 3);
  /// ```
  fn with_ref<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    let guard = self.borrow();
    f(&*guard)
  }

  /// 状態への可変参照を使用してクロージャを実行します。
  ///
  /// このメソッドは、状態を可変的に借用し、その参照をクロージャに渡して実行します。
  /// クロージャの実行が完了すると、自動的にロックが解放されます。
  /// 手動でガードを管理する必要がないため、より安全で簡潔なコードが書けます。
  ///
  /// # 引数
  ///
  /// * `f` - 状態への可変参照を受け取り、任意の型`R`を返すクロージャ
  ///
  /// # 戻り値
  ///
  /// クロージャの実行結果
  ///
  /// # 例
  ///
  /// ```rust,ignore
  /// let cell = RcState::new(0);
  /// cell.with_ref_mut(|v| *v += 1);
  /// assert_eq!(cell.with_ref(|v| *v), 1);
  /// ```
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
