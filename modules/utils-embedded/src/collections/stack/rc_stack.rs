use core::cell::RefCell;

use nexus_utils_core_rs::{
  QueueSize, Stack, StackBase, StackBuffer, StackError, StackMut, StackStorage, StackStorageBackend,
};

use crate::sync::RcShared;

/// `Rc`ベースのスタックストレージ型エイリアス
///
/// `RcShared`と`RefCell`を使用した参照カウントベースのスタックストレージです。
type RcStackStorage<T> = RcShared<StackStorageBackend<RcShared<RefCell<StackBuffer<T>>>>>;

/// `Rc`ベースのスタック（後入れ先出しデータ構造）
///
/// このスタックは`no_std`環境で利用可能な、LIFO(Last-In-First-Out)データ構造です。
/// `Rc`と`RefCell`を使用して参照カウントベースの共有所有権を提供します。
///
/// # 特徴
///
/// - **LIFO**: 最後に追加された要素が最初に取り出されます
/// - **容量制限**: オプションで容量制限を設定可能
/// - **no_std対応**: 標準ライブラリを必要としません
/// - **クローン可能**: `clone()`で複数のハンドルを作成可能
///
/// # パフォーマンス特性
///
/// - `push`: O(1)（容量内）、リサイズ時はO(n)
/// - `pop`: O(1)
/// - `peek`: O(1)
/// - メモリ使用量: O(n)（要素数に比例）
///
/// # 例
///
/// ```
/// use nexus_utils_embedded_rs::RcStack;
///
/// // 容量制限なしのスタック
/// let stack = RcStack::new();
/// stack.push(1).unwrap();
/// stack.push(2).unwrap();
/// assert_eq!(stack.pop(), Some(2));
/// assert_eq!(stack.pop(), Some(1));
///
/// // 容量制限付きのスタック
/// let limited_stack = RcStack::with_capacity(5);
/// for i in 0..5 {
///     limited_stack.push(i).unwrap();
/// }
/// // 6番目の要素は追加できない
/// assert!(limited_stack.push(5).is_err());
/// ```
#[derive(Debug, Clone)]
pub struct RcStack<T> {
  inner: Stack<RcStackStorage<T>, T>,
}

impl<T> RcStack<T> {
  /// 新しいスタックを作成します
  ///
  /// 容量制限なしで作成されます。
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack: RcStack<String> = RcStack::new();
  /// ```
  pub fn new() -> Self {
    let storage = RcShared::new(RefCell::new(StackBuffer::new()));
    let backend: RcStackStorage<T> = RcShared::new(StackStorageBackend::new(storage));
    Self {
      inner: Stack::new(backend),
    }
  }

  /// 指定された容量制限で新しいスタックを作成します
  ///
  /// # 引数
  ///
  /// * `capacity` - スタックに格納できる最大要素数
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack: RcStack<i32> = RcStack::with_capacity(100);
  /// ```
  pub fn with_capacity(capacity: usize) -> Self {
    let stack = Self::new();
    stack.set_capacity(Some(capacity));
    stack
  }

  /// スタックの容量制限を設定します
  ///
  /// # 引数
  ///
  /// * `capacity` - `Some(n)`で容量をn要素に制限、`None`で容量制限を解除
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack: RcStack<i32> = RcStack::new();
  /// stack.set_capacity(Some(10)); // 容量を10に制限
  /// stack.set_capacity(None);     // 容量制限を解除
  /// ```
  pub fn set_capacity(&self, capacity: Option<usize>) {
    self.inner.set_capacity(capacity);
  }

  /// スタックに値を追加します
  ///
  /// # 引数
  ///
  /// * `value` - スタックに追加する値
  ///
  /// # 戻り値
  ///
  /// 成功した場合は`Ok(())`、容量制限に達している場合は`Err(StackError::Full(value))`
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack = RcStack::new();
  /// stack.push(42).unwrap();
  /// ```
  pub fn push(&self, value: T) -> Result<(), StackError<T>> {
    self.inner.push(value)
  }

  /// スタックから値を取り出します
  ///
  /// # 戻り値
  ///
  /// スタックが空でない場合は`Some(value)`、空の場合は`None`
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack = RcStack::new();
  /// stack.push(1).unwrap();
  /// assert_eq!(stack.pop(), Some(1));
  /// assert_eq!(stack.pop(), None);
  /// ```
  pub fn pop(&self) -> Option<T> {
    self.inner.pop()
  }

  /// スタックのトップの値を取り出さずに参照します
  ///
  /// # 戻り値
  ///
  /// スタックが空でない場合は`Some(value)`、空の場合は`None`
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack = RcStack::new();
  /// stack.push(1).unwrap();
  /// assert_eq!(stack.peek(), Some(1));
  /// assert_eq!(stack.len().to_usize(), 1); // 要素はまだ残っている
  /// ```
  pub fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.inner.peek()
  }

  /// スタックからすべての要素を削除します
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack = RcStack::new();
  /// stack.push(1).unwrap();
  /// stack.push(2).unwrap();
  /// stack.clear();
  /// assert_eq!(stack.len().to_usize(), 0);
  /// ```
  pub fn clear(&self) {
    self.inner.clear();
  }

  /// スタック内の要素数を返します
  ///
  /// # 戻り値
  ///
  /// 現在の要素数を表す`QueueSize`
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack = RcStack::new();
  /// stack.push(1).unwrap();
  /// assert_eq!(stack.len().to_usize(), 1);
  /// ```
  pub fn len(&self) -> QueueSize {
    self.inner.len()
  }

  /// スタックの容量を返します
  ///
  /// # 戻り値
  ///
  /// 容量制限が設定されている場合は制限値、設定されていない場合は`QueueSize::Limitless`
  ///
  /// # 例
  ///
  /// ```
  /// use nexus_utils_embedded_rs::RcStack;
  ///
  /// let stack = RcStack::with_capacity(10);
  /// assert_eq!(stack.capacity().to_usize(), 10);
  /// ```
  pub fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T> Default for RcStack<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> StackBase<T> for RcStack<T> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T> StackMut<T> for RcStack<T> {
  fn push(&mut self, value: T) -> Result<(), StackError<T>> {
    self.inner.push(value)
  }

  fn pop(&mut self) -> Option<T> {
    self.inner.pop()
  }

  fn clear(&mut self) {
    self.inner.clear();
  }

  fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.inner.peek()
  }
}

impl<T> StackStorage<T> for RcShared<RefCell<StackBuffer<T>>> {
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
    f(&self.borrow())
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
    f(&mut self.borrow_mut())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rc_stack_push_pop() {
    let stack = RcStack::with_capacity(1);
    stack.push(1).unwrap();
    assert!(stack.push(2).is_err());
    assert_eq!(stack.pop(), Some(1));
    assert!(stack.pop().is_none());
  }

  #[test]
  fn rc_stack_handle_operations() {
    let stack = RcStack::new();
    stack.push(10).unwrap();
    let cloned = stack.clone();
    cloned.push(11).unwrap();

    assert_eq!(stack.len().to_usize(), 2);
    assert_eq!(cloned.pop(), Some(11));
    assert_eq!(stack.pop(), Some(10));
  }

  #[test]
  fn rc_stack_peek_ref() {
    let stack = RcStack::new();
    stack.push(5).unwrap();
    assert_eq!(stack.peek(), Some(5));
    stack.pop();
    assert_eq!(stack.peek(), None);
  }
}
