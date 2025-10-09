use crate::collections::stack::buffer::StackBuffer;
use crate::collections::QueueSize;
use crate::sync::Shared;

use super::StackError;

/// スタックバックエンドで使用されるストレージの抽象化。
///
/// このトレイトは、スタックの内部データ構造へのアクセスを提供し、
/// 読み取り専用および書き込み可能な操作を可能にします。
pub trait StackStorage<T> {
  /// 読み取り専用アクセスでクロージャを実行します。
  ///
  /// # 引数
  ///
  /// * `f` - スタックバッファへの不変参照を受け取るクロージャ
  ///
  /// # 戻り値
  ///
  /// クロージャの実行結果
  fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R;

  /// 書き込み可能アクセスでクロージャを実行します。
  ///
  /// # 引数
  ///
  /// * `f` - スタックバッファへの可変参照を受け取るクロージャ
  ///
  /// # 戻り値
  ///
  /// クロージャの実行結果
  fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R;
}

/// スタック操作のためのバックエンド抽象化。
///
/// このトレイトは、スタックの基本操作(push、pop、peek等)を定義し、
/// 具体的なストレージ実装から独立したインターフェースを提供します。
pub trait StackBackend<T> {
  /// スタックに値をプッシュします。
  ///
  /// # 引数
  ///
  /// * `value` - プッシュする値
  ///
  /// # 戻り値
  ///
  /// * `Ok(())` - 成功時
  /// * `Err(StackError<T>)` - 容量制限に達した場合
  fn push(&self, value: T) -> Result<(), StackError<T>>;

  /// スタックから値をポップします。
  ///
  /// # 戻り値
  ///
  /// * `Some(T)` - スタックが空でない場合、最後の値
  /// * `None` - スタックが空の場合
  fn pop(&self) -> Option<T>;

  /// スタックのすべての要素をクリアします。
  fn clear(&self);

  /// スタックの現在の要素数を取得します。
  ///
  /// # 戻り値
  ///
  /// 要素数を表す`QueueSize`
  fn len(&self) -> QueueSize;

  /// スタックの容量を取得します。
  ///
  /// # 戻り値
  ///
  /// 容量を表す`QueueSize`(無制限の場合は`QueueSize::Unlimited`)
  fn capacity(&self) -> QueueSize;

  /// スタックの容量を設定します。
  ///
  /// # 引数
  ///
  /// * `capacity` - 新しい容量(`None`の場合は無制限)
  fn set_capacity(&self, capacity: Option<usize>);

  /// スタックが空かどうかを判定します。
  ///
  /// # 戻り値
  ///
  /// * `true` - スタックが空の場合
  /// * `false` - スタックに要素がある場合
  fn is_empty(&self) -> bool {
    self.len() == QueueSize::Limited(0)
  }

  /// スタックの最上位の値を取得します(ポップはしません)。
  ///
  /// # 戻り値
  ///
  /// * `Some(T)` - スタックが空でない場合、最上位の値のクローン
  /// * `None` - スタックが空の場合
  fn peek(&self) -> Option<T>
  where
    T: Clone;
}

/// [`StackBackend`]を公開するハンドル。
///
/// このトレイトは、スタックバックエンドへの共有アクセスを提供し、
/// 複数の所有者が同じスタックインスタンスを使用できるようにします。
pub trait StackHandle<T>: Shared<Self::Backend> + Clone {
  /// このハンドルが管理するバックエンドの型。
  type Backend: StackBackend<T> + ?Sized;

  /// バックエンドへの参照を取得します。
  ///
  /// # 戻り値
  ///
  /// スタックバックエンドへの参照
  fn backend(&self) -> &Self::Backend;
}

/// [`StackStorage`]上で直接動作するバックエンド実装。
///
/// このバックエンドは、ストレージ抽象化を使用してスタック操作を実装します。
#[derive(Debug)]
pub struct StackStorageBackend<S> {
  storage: S,
}

impl<S> StackStorageBackend<S> {
  /// 指定されたストレージで新しい`StackStorageBackend`を作成します。
  ///
  /// # 引数
  ///
  /// * `storage` - 使用するストレージ実装
  pub const fn new(storage: S) -> Self {
    Self { storage }
  }

  /// ストレージへの参照を取得します。
  ///
  /// # 戻り値
  ///
  /// 内部ストレージへの参照
  pub fn storage(&self) -> &S {
    &self.storage
  }

  /// バックエンドを消費してストレージを取り出します。
  ///
  /// # 戻り値
  ///
  /// 内部ストレージの所有権
  pub fn into_storage(self) -> S {
    self.storage
  }
}

impl<S, T> StackBackend<T> for StackStorageBackend<S>
where
  S: StackStorage<T>,
{
  fn push(&self, value: T) -> Result<(), StackError<T>> {
    self.storage.with_write(|buffer| buffer.push(value))
  }

  fn pop(&self) -> Option<T> {
    self.storage.with_write(|buffer| buffer.pop())
  }

  fn clear(&self) {
    self.storage.with_write(|buffer| buffer.clear());
  }

  fn len(&self) -> QueueSize {
    self.storage.with_read(|buffer| buffer.len())
  }

  fn capacity(&self) -> QueueSize {
    self.storage.with_read(|buffer| buffer.capacity())
  }

  fn set_capacity(&self, capacity: Option<usize>) {
    self.storage.with_write(|buffer| buffer.set_capacity(capacity));
  }

  fn peek(&self) -> Option<T>
  where
    T: Clone, {
    self.storage.with_read(|buffer| buffer.peek().cloned())
  }
}

/// スタック型コレクションの基本トレイト。
///
/// このトレイトは、スタックの基本的な情報取得メソッドを定義します。
pub trait StackBase<T> {
  /// スタックの現在の要素数を取得します。
  ///
  /// # 戻り値
  ///
  /// 要素数を表す`QueueSize`
  fn len(&self) -> QueueSize;

  /// スタックの容量を取得します。
  ///
  /// # 戻り値
  ///
  /// 容量を表す`QueueSize`(無制限の場合は`QueueSize::Unlimited`)
  fn capacity(&self) -> QueueSize;

  /// スタックが空かどうかを判定します。
  ///
  /// # 戻り値
  ///
  /// * `true` - スタックが空の場合
  /// * `false` - スタックに要素がある場合
  fn is_empty(&self) -> bool {
    self.len().to_usize() == 0
  }
}

/// 可変スタックインターフェース。
///
/// このトレイトは、スタックの変更操作(push、pop、clear等)を提供します。
pub trait StackMut<T>: StackBase<T> {
  /// スタックに値をプッシュします。
  ///
  /// # 引数
  ///
  /// * `value` - プッシュする値
  ///
  /// # 戻り値
  ///
  /// * `Ok(())` - 成功時
  /// * `Err(StackError<T>)` - 容量制限に達した場合
  fn push(&mut self, value: T) -> Result<(), StackError<T>>;

  /// スタックから値をポップします。
  ///
  /// # 戻り値
  ///
  /// * `Some(T)` - スタックが空でない場合、最後の値
  /// * `None` - スタックが空の場合
  fn pop(&mut self) -> Option<T>;

  /// スタックのすべての要素をクリアします。
  fn clear(&mut self);

  /// スタックの最上位の値を取得します(ポップはしません)。
  ///
  /// # 戻り値
  ///
  /// * `Some(T)` - スタックが空でない場合、最上位の値のクローン
  /// * `None` - スタックが空の場合
  fn peek(&self) -> Option<T>
  where
    T: Clone;
}

#[cfg(feature = "alloc")]
mod alloc_impls {
  use core::cell::RefCell;

  use super::{StackBuffer, StackStorage};

  impl<T> StackStorage<T> for RefCell<StackBuffer<T>> {
    fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
      f(&self.borrow())
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
      f(&mut self.borrow_mut())
    }
  }
}

#[cfg(all(feature = "alloc", feature = "std"))]
mod std_impls {
  use std::sync::Mutex;

  use super::{StackBuffer, StackStorage};

  impl<T> StackStorage<T> for Mutex<StackBuffer<T>> {
    fn with_read<R>(&self, f: impl FnOnce(&StackBuffer<T>) -> R) -> R {
      let guard = self.lock().expect("mutex poisoned");
      f(&guard)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut StackBuffer<T>) -> R) -> R {
      let mut guard = self.lock().expect("mutex poisoned");
      f(&mut guard)
    }
  }
}
