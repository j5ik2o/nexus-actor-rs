use super::traits::{StackBackend, StackBase, StackHandle, StackMut};
use crate::collections::QueueSize;

use super::StackError;

/// [`StackBackend`]に処理を委譲するスタックファサード。
///
/// このstructは、スタックの操作を抽象化し、実際のバックエンド実装への
/// 統一的なインターフェースを提供します。
///
/// # 型パラメータ
///
/// * `H` - [`StackHandle`]を実装したバックエンドハンドル型
/// * `T` - スタックに格納する要素の型
///
/// # Examples
///
/// ```ignore
/// use nexus_actor_utils_rs::collections::stack::{Stack, StackStorageBackend};
///
/// let backend = /* StackHandle実装を作成 */;
/// let stack = Stack::new(backend);
/// stack.push(42).unwrap();
/// assert_eq!(stack.pop(), Some(42));
/// ```
#[derive(Debug)]
pub struct Stack<H, T>
where
  H: StackHandle<T>, {
  backend: H,
  _marker: core::marker::PhantomData<T>,
}

impl<H, T> Stack<H, T>
where
  H: StackHandle<T>,
{
  /// 指定されたバックエンドハンドルから新しい[`Stack`]を作成します。
  ///
  /// # 引数
  ///
  /// * `backend` - スタック操作を処理するバックエンドハンドル
  ///
  /// # Returns
  ///
  /// 新しい[`Stack`]インスタンス
  pub fn new(backend: H) -> Self {
    Self {
      backend,
      _marker: core::marker::PhantomData,
    }
  }

  /// バックエンドハンドルへの参照を取得します。
  ///
  /// # Returns
  ///
  /// バックエンドハンドルへの不変参照
  pub fn backend(&self) -> &H {
    &self.backend
  }

  /// [`Stack`]を消費して、内部のバックエンドハンドルを取得します。
  ///
  /// # Returns
  ///
  /// 内部に保持していたバックエンドハンドル
  pub fn into_backend(self) -> H {
    self.backend
  }

  /// スタックの容量制限を設定します。
  ///
  /// # 引数
  ///
  /// * `capacity` - 最大容量。`None`の場合は無制限
  pub fn set_capacity(&self, capacity: Option<usize>) {
    self.backend.backend().set_capacity(capacity);
  }

  /// スタックの先頭に値をプッシュします。
  ///
  /// 容量制限がある場合、スタックが満杯だと[`StackError::Full`]を返します。
  ///
  /// # 引数
  ///
  /// * `value` - プッシュする値
  ///
  /// # Returns
  ///
  /// * `Ok(())` - プッシュが成功
  /// * `Err(StackError<T>)` - スタックが満杯の場合、元の値を含むエラー
  pub fn push(&self, value: T) -> Result<(), StackError<T>> {
    self.backend.backend().push(value)
  }

  /// スタックの先頭から値をポップします。
  ///
  /// # Returns
  ///
  /// * `Some(T)` - ポップされた値
  /// * `None` - スタックが空の場合
  pub fn pop(&self) -> Option<T> {
    self.backend.backend().pop()
  }

  /// スタックの先頭の値を削除せずに参照します。
  ///
  /// この操作は要素をクローンするため、`T`は[`Clone`]を実装している必要があります。
  ///
  /// # Returns
  ///
  /// * `Some(T)` - 先頭の値のクローン
  /// * `None` - スタックが空の場合
  pub fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.backend.backend().peek()
  }

  /// スタック内のすべての要素をクリアします。
  pub fn clear(&self) {
    self.backend.backend().clear();
  }

  /// スタックに格納されている要素数を取得します。
  ///
  /// # Returns
  ///
  /// 現在の要素数を表す[`QueueSize`]
  pub fn len(&self) -> QueueSize {
    self.backend.backend().len()
  }

  /// スタックの容量を取得します。
  ///
  /// # Returns
  ///
  /// * `QueueSize::Bounded(n)` - 最大容量が`n`
  /// * `QueueSize::Unbounded` - 無制限
  pub fn capacity(&self) -> QueueSize {
    self.backend.backend().capacity()
  }
}

impl<H, T> Clone for Stack<H, T>
where
  H: StackHandle<T>,
{
  /// [`Stack`]のクローンを作成します。
  ///
  /// バックエンドハンドルをクローンすることで、同じスタックを参照する
  /// 新しいインスタンスを作成します。
  fn clone(&self) -> Self {
    Self {
      backend: self.backend.clone(),
      _marker: core::marker::PhantomData,
    }
  }
}

/// [`StackBase`]トレイトの実装。
///
/// スタックの基本的な照会操作（長さと容量の取得）を提供します。
impl<H, T> StackBase<T> for Stack<H, T>
where
  H: StackHandle<T>,
{
  fn len(&self) -> QueueSize {
    self.backend.backend().len()
  }

  fn capacity(&self) -> QueueSize {
    self.backend.backend().capacity()
  }
}

/// [`StackMut`]トレイトの実装。
///
/// 可変参照を通じてスタックを操作するメソッドを提供します。
/// 非可変メソッドと同じ操作を、`&mut self`レシーバで実行できます。
impl<H, T> StackMut<T> for Stack<H, T>
where
  H: StackHandle<T>,
{
  fn push(&mut self, value: T) -> Result<(), StackError<T>> {
    self.backend.backend().push(value)
  }

  fn pop(&mut self) -> Option<T> {
    self.backend.backend().pop()
  }

  fn clear(&mut self) {
    self.backend.backend().clear();
  }

  fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.backend.backend().peek()
  }
}

#[cfg(test)]
mod tests {
  extern crate alloc;
  use alloc::rc::Rc;
  use core::cell::RefCell;

  use super::*;
  use crate::collections::stack::buffer::StackBuffer;
  use crate::collections::stack::traits::{StackHandle, StackStorage, StackStorageBackend};
  use crate::sync::Shared;

  struct RcStorageHandle<T>(Rc<RefCell<StackBuffer<T>>>);

  impl<T> Clone for RcStorageHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> core::ops::Deref for RcStorageHandle<T> {
    type Target = RefCell<StackBuffer<T>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> StackStorage<T> for RcStorageHandle<T> {
    fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
      f(&self.borrow())
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
      f(&mut self.borrow_mut())
    }
  }

  impl<T> Shared<RefCell<StackBuffer<T>>> for RcStorageHandle<T> {}

  struct RcBackendHandle<T>(Rc<StackStorageBackend<RcStorageHandle<T>>>);

  impl<T> Clone for RcBackendHandle<T> {
    fn clone(&self) -> Self {
      Self(self.0.clone())
    }
  }

  impl<T> core::ops::Deref for RcBackendHandle<T> {
    type Target = StackStorageBackend<RcStorageHandle<T>>;

    fn deref(&self) -> &Self::Target {
      &self.0
    }
  }

  impl<T> Shared<StackStorageBackend<RcStorageHandle<T>>> for RcBackendHandle<T> {}

  impl<T> StackHandle<T> for RcBackendHandle<T> {
    type Backend = StackStorageBackend<RcStorageHandle<T>>;

    fn backend(&self) -> &Self::Backend {
      &self.0
    }
  }

  #[test]
  fn stack_push_pop_via_handle() {
    let storage = RcStorageHandle(Rc::new(RefCell::new(StackBuffer::new())));
    let backend = RcBackendHandle(Rc::new(StackStorageBackend::new(storage)));
    let stack = Stack::new(backend.clone());

    stack.set_capacity(Some(2));
    stack.push(1).unwrap();
    stack.push(2).unwrap();
    assert!(stack.push(3).is_err());
    assert_eq!(stack.pop(), Some(2));
    assert_eq!(backend.backend().len().to_usize(), 1);
  }

  #[test]
  fn stack_peek_via_handle() {
    let storage = RcStorageHandle(Rc::new(RefCell::new(StackBuffer::new())));
    let backend = RcBackendHandle(Rc::new(StackStorageBackend::new(storage)));
    let stack = Stack::new(backend);

    stack.push(7).unwrap();
    assert_eq!(stack.peek(), Some(7));
    stack.pop();
    assert_eq!(stack.peek(), None);
  }

  #[test]
  fn stack_clear_via_handle() {
    let storage = RcStorageHandle(Rc::new(RefCell::new(StackBuffer::new())));
    let backend = RcBackendHandle(Rc::new(StackStorageBackend::new(storage)));
    let stack = Stack::new(backend);

    stack.push(1).unwrap();
    stack.clear();
    assert!(stack.is_empty());
  }
}
