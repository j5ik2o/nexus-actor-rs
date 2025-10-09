use std::sync::Mutex;

use crate::sync::ArcShared;
use nexus_utils_core_rs::{
  QueueSize, Stack, StackBase, StackBuffer, StackError, StackMut, StackStorage, StackStorageBackend,
};

type ArcStackStorage<T> = ArcShared<StackStorageBackend<ArcShared<Mutex<StackBuffer<T>>>>>;

/// `Arc`ベースのスレッドセーフなスタック実装
///
/// 複数のスレッド間で安全に共有可能なスタックデータ構造を提供します。
/// 内部的に`Arc`と`Mutex`を使用して同期を行います。
#[derive(Debug, Clone)]
pub struct ArcStack<T> {
  inner: Stack<ArcStackStorage<T>, T>,
}

impl<T> ArcStack<T> {
  /// 新しい`ArcStack`を作成します
  ///
  /// # Returns
  ///
  /// 空の`ArcStack`インスタンス
  pub fn new() -> Self {
    let storage = ArcShared::new(Mutex::new(StackBuffer::new()));
    let backend: ArcStackStorage<T> = ArcShared::new(StackStorageBackend::new(storage));
    Self {
      inner: Stack::new(backend),
    }
  }

  /// 指定された容量を持つ`ArcStack`を作成します
  ///
  /// # Arguments
  ///
  /// * `capacity` - スタックの最大容量
  ///
  /// # Returns
  ///
  /// 指定された容量を持つ空の`ArcStack`インスタンス
  pub fn with_capacity(capacity: usize) -> Self {
    let stack = Self::new();
    stack.set_capacity(Some(capacity));
    stack
  }

  /// スタックの容量を設定します
  ///
  /// # Arguments
  ///
  /// * `capacity` - スタックの最大容量。`None`の場合は無制限
  pub fn set_capacity(&self, capacity: Option<usize>) {
    self.inner.set_capacity(capacity);
  }

  /// スタックに値をプッシュします
  ///
  /// # Arguments
  ///
  /// * `value` - プッシュする値
  ///
  /// # Returns
  ///
  /// 成功時は`Ok(())`、容量制限を超える場合は`Err(StackError)`
  ///
  /// # Errors
  ///
  /// スタックが容量制限に達している場合、`StackError`を返します
  pub fn push(&self, value: T) -> Result<(), StackError<T>> {
    self.inner.push(value)
  }

  /// スタックから値をポップします
  ///
  /// # Returns
  ///
  /// スタックが空でない場合は`Some(T)`、空の場合は`None`
  pub fn pop(&self) -> Option<T> {
    self.inner.pop()
  }

  /// スタックの先頭要素を削除せずに取得します
  ///
  /// # Returns
  ///
  /// スタックが空でない場合は先頭要素のクローン`Some(T)`、空の場合は`None`
  pub fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.inner.peek()
  }

  /// スタック内のすべての要素を削除します
  pub fn clear(&self) {
    self.inner.clear();
  }

  /// スタック内の要素数を取得します
  ///
  /// # Returns
  ///
  /// 現在のスタック内の要素数
  pub fn len(&self) -> QueueSize {
    self.inner.len()
  }

  /// スタックの容量を取得します
  ///
  /// # Returns
  ///
  /// スタックの最大容量。無制限の場合は`QueueSize::Unlimited`
  pub fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T> Default for ArcStack<T> {
  fn default() -> Self {
    Self::new()
  }
}

impl<T> StackBase<T> for ArcStack<T> {
  fn len(&self) -> QueueSize {
    self.inner.len()
  }

  fn capacity(&self) -> QueueSize {
    self.inner.capacity()
  }
}

impl<T> StackMut<T> for ArcStack<T> {
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

impl<T> StackStorage<T> for ArcShared<Mutex<StackBuffer<T>>> {
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
    let guard = self.lock().expect("mutex poisoned");
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
    let mut guard = self.lock().expect("mutex poisoned");
    f(&mut guard)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn stack_push_pop() {
    let stack = ArcStack::with_capacity(1);
    stack.push(1).unwrap();
    assert!(stack.push(2).is_err());
    assert_eq!(stack.pop(), Some(1));
    assert!(stack.pop().is_none());
  }

  #[test]
  fn stack_handle_access() {
    let stack = ArcStack::new();
    stack.push(10).unwrap();
    let cloned = stack.clone();
    cloned.push(11).unwrap();

    assert_eq!(stack.len().to_usize(), 2);
    assert_eq!(cloned.pop(), Some(11));
    assert_eq!(stack.pop(), Some(10));
  }

  #[test]
  fn stack_peek_ref() {
    let stack = ArcStack::new();
    stack.push(5).unwrap();
    assert_eq!(stack.peek(), Some(5));
    stack.pop();
    assert_eq!(stack.peek(), None);
  }
}
