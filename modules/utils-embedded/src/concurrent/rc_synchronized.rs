use alloc::boxed::Box;
use alloc::rc::Rc;

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::{Mutex, MutexGuard};
use embassy_sync::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use nexus_utils_core_rs::{
  async_trait, Synchronized as CoreSynchronized, SynchronizedMutexBackend, SynchronizedRw as CoreSynchronizedRw,
  SynchronizedRwBackend,
};

/// `Rc` + `Mutex` による同期化バックエンド。
///
/// `no_std` 環境で、排他的アクセスを提供する同期プリミティブを実装します。
/// Embassy の `Mutex` を使用して、値への非同期的な排他アクセスを実現します。
///
/// # 特徴
///
/// - `Rc` による参照カウント（シングルスレッド専用）
/// - Embassy の `NoopRawMutex` による軽量な排他制御
/// - 読み書き両方で同じロックを使用
#[derive(Clone, Debug)]
pub struct RcMutexBackend<T> {
  inner: Rc<Mutex<NoopRawMutex, T>>,
}

#[async_trait(?Send)]
impl<T> SynchronizedMutexBackend<T> for RcMutexBackend<T>
where
  T: 'static,
{
  type Guard<'a>
    = MutexGuard<'a, NoopRawMutex, T>
  where
    Self: 'a;

  /// 指定された値で新しい同期化バックエンドを作成します。
  fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      inner: Rc::new(Mutex::new(value)),
    }
  }

  /// ロックを取得し、ガードを返します。
  ///
  /// 他のタスクがロックを保持している場合、解放されるまで待機します。
  async fn lock(&self) -> Self::Guard<'_> {
    self.inner.lock().await
  }
}

/// `Rc` + `RwLock` による読み取り/書き込み同期化バックエンド。
///
/// `no_std` 環境で、複数の読み取りまたは単一の書き込みアクセスを提供する同期プリミティブを実装します。
/// Embassy の `RwLock` を使用して、値への非同期的な読み取り/書き込みアクセスを実現します。
///
/// # 特徴
///
/// - `Rc` による参照カウント（シングルスレッド専用）
/// - Embassy の `NoopRawMutex` による軽量なロック機構
/// - 複数の読み取りまたは単一の書き込みを許可
#[derive(Clone, Debug)]
pub struct RcRwLockBackend<T> {
  inner: Rc<RwLock<NoopRawMutex, T>>,
}

#[async_trait(?Send)]
impl<T> SynchronizedRwBackend<T> for RcRwLockBackend<T>
where
  T: 'static,
{
  type ReadGuard<'a>
    = RwLockReadGuard<'a, NoopRawMutex, T>
  where
    Self: 'a;
  type WriteGuard<'a>
    = RwLockWriteGuard<'a, NoopRawMutex, T>
  where
    Self: 'a;

  /// 指定された値で新しい読み取り/書き込み同期化バックエンドを作成します。
  fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      inner: Rc::new(RwLock::new(value)),
    }
  }

  /// 読み取りロックを取得し、読み取りガードを返します。
  ///
  /// 複数の読み取りロックは同時に保持できます。
  /// 書き込みロックが保持されている場合、解放されるまで待機します。
  async fn read(&self) -> Self::ReadGuard<'_> {
    self.inner.read().await
  }

  /// 書き込みロックを取得し、書き込みガードを返します。
  ///
  /// 書き込みロックは排他的であり、他の読み取り/書き込みロックが解放されるまで待機します。
  async fn write(&self) -> Self::WriteGuard<'_> {
    self.inner.write().await
  }
}

/// `Rc` ベースの排他的同期型の型エイリアス。
///
/// `no_std` 環境で使用できる、排他的アクセス制御を提供する同期プリミティブです。
/// 読み取り/書き込み両方で同じロックを使用します。
pub type Synchronized<T> = CoreSynchronized<RcMutexBackend<T>, T>;

/// `Rc` ベースの読み取り/書き込み同期型の型エイリアス。
///
/// `no_std` 環境で使用できる、複数の読み取りまたは単一の書き込みアクセスを提供する同期プリミティブです。
pub type SynchronizedRw<T> = CoreSynchronizedRw<RcRwLockBackend<T>, T>;

#[cfg(test)]
mod tests {
  use super::{Synchronized, SynchronizedRw};
  use alloc::vec;
  use futures::executor::block_on;

  #[test]
  fn rc_mutex_backend_basic() {
    block_on(async {
      let sync = Synchronized::new(1);
      let value = sync.read(|guard| **guard).await;
      assert_eq!(value, 1);

      sync
        .write(|guard| {
          **guard = 7;
        })
        .await;

      let updated = {
        let guard = sync.lock().await;
        let guard = guard.into_inner();
        *guard
      };
      assert_eq!(updated, 7);
    });
  }

  #[test]
  fn rc_rw_backend_behaviour() {
    block_on(async {
      let sync = SynchronizedRw::new(vec![1]);
      let len = sync.read(|guard| guard.len()).await;
      assert_eq!(len, 1);

      sync
        .write(|guard| {
          guard.push(2);
        })
        .await;

      let sum = {
        let guard = sync.read_guard().await;
        guard.iter().sum::<i32>()
      };
      assert_eq!(sum, 3);
    });
  }
}
