use super::{mpsc::MpscBuffer, ring::RingBuffer};

/// キューのストレージ抽象化トレイト
///
/// リングバッファベースのキューに対する読み取りおよび書き込みアクセスを提供します。
/// このトレイトは、異なる同期プリミティブ（RefCell、Mutexなど）でラップされた
/// リングバッファに対して統一的なインターフェースを提供します。
///
/// # 型パラメータ
///
/// * `E` - キューに格納される要素の型
pub trait QueueStorage<E> {
  /// リングバッファへの不変参照を使用してクロージャを実行します
  ///
  /// # Arguments
  ///
  /// * `f` - リングバッファの不変参照を受け取るクロージャ
  ///
  /// # Returns
  ///
  /// クロージャの実行結果
  fn with_read<R>(&self, f: impl FnOnce(&RingBuffer<E>) -> R) -> R;

  /// リングバッファへの可変参照を使用してクロージャを実行します
  ///
  /// # Arguments
  ///
  /// * `f` - リングバッファの可変参照を受け取るクロージャ
  ///
  /// # Returns
  ///
  /// クロージャの実行結果
  fn with_write<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R;
}

/// リングバッファベースのストレージ抽象化トレイト
///
/// [`crate::collections::queue::mpsc::RingBufferBackend`] 実装で共有される
/// ストレージ抽象化を提供します。このトレイトは、MPSCバッファに対する
/// 読み取りおよび書き込みアクセスを統一的に扱うためのインターフェースです。
///
/// # 型パラメータ
///
/// * `T` - バッファに格納される要素の型
pub trait RingBufferStorage<T> {
  /// MPSCバッファへの不変参照を使用してクロージャを実行します
  ///
  /// # Arguments
  ///
  /// * `f` - MPSCバッファの不変参照を受け取るクロージャ
  ///
  /// # Returns
  ///
  /// クロージャの実行結果
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R;

  /// MPSCバッファへの可変参照を使用してクロージャを実行します
  ///
  /// # Arguments
  ///
  /// * `f` - MPSCバッファの可変参照を受け取るクロージャ
  ///
  /// # Returns
  ///
  /// クロージャの実行結果
  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R;
}

#[cfg(feature = "alloc")]
mod queue_alloc_impls {
  use core::cell::RefCell;

  use super::{QueueStorage, RingBuffer};

  impl<E> QueueStorage<E> for RefCell<RingBuffer<E>> {
    fn with_read<R>(&self, f: impl FnOnce(&RingBuffer<E>) -> R) -> R {
      let guard = self.borrow();
      f(&guard)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R {
      let mut guard = self.borrow_mut();
      f(&mut guard)
    }
  }
}

#[cfg(all(feature = "alloc", feature = "std"))]
mod queue_std_impls {
  use std::sync::Mutex;

  use super::{QueueStorage, RingBuffer};

  impl<E> QueueStorage<E> for Mutex<RingBuffer<E>> {
    fn with_read<R>(&self, f: impl FnOnce(&RingBuffer<E>) -> R) -> R {
      let guard = self.lock().expect("mutex poisoned");
      f(&guard)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R {
      let mut guard = self.lock().expect("mutex poisoned");
      f(&mut guard)
    }
  }
}

#[cfg(feature = "alloc")]
mod mpsc_alloc_impls {
  use core::cell::RefCell;

  use super::{MpscBuffer, RingBufferStorage};

  impl<T> RingBufferStorage<T> for RefCell<MpscBuffer<T>> {
    fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
      let guard = self.borrow();
      f(&guard)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
      let mut guard = self.borrow_mut();
      f(&mut guard)
    }
  }
}

#[cfg(all(feature = "alloc", feature = "std"))]
mod mpsc_std_impls {
  use std::sync::Mutex;

  use super::{MpscBuffer, RingBufferStorage};

  impl<T> RingBufferStorage<T> for Mutex<MpscBuffer<T>> {
    fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
      let guard = self.lock().expect("mutex poisoned");
      f(&guard)
    }

    fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
      let mut guard = self.lock().expect("mutex poisoned");
      f(&mut guard)
    }
  }
}
