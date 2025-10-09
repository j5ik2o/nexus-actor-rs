use nexus_utils_core_rs::{
  async_trait, Synchronized as CoreSynchronized, SynchronizedMutexBackend, SynchronizedRw as CoreSynchronizedRw,
  SynchronizedRwBackend,
};
use tokio::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Tokio Mutexを使用した排他制御バックエンド実装
///
/// 共有データへの排他的アクセスを提供します。
pub struct TokioMutexBackend<T> {
  inner: Mutex<T>,
}

impl<T> TokioMutexBackend<T> {
  /// 既存のTokio Mutexから新しいバックエンドインスタンスを作成します。
  ///
  /// # 引数
  ///
  /// * `inner` - ラップするTokio Mutex
  ///
  /// # 戻り値
  ///
  /// 新しい`TokioMutexBackend`インスタンス
  pub fn new_with_mutex(inner: Mutex<T>) -> Self {
    Self { inner }
  }
}

#[async_trait(?Send)]
impl<T> SynchronizedMutexBackend<T> for TokioMutexBackend<T>
where
  T: Send,
{
  type Guard<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a;

  fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      inner: Mutex::new(value),
    }
  }

  async fn lock(&self) -> Self::Guard<'_> {
    self.inner.lock().await
  }
}

/// Tokio RwLockを使用した読み書きロックバックエンド実装
///
/// 複数の読み取りアクセスまたは単一の書き込みアクセスを提供します。
pub struct TokioRwLockBackend<T> {
  inner: RwLock<T>,
}

impl<T> TokioRwLockBackend<T> {
  /// 既存のTokio RwLockから新しいバックエンドインスタンスを作成します。
  ///
  /// # 引数
  ///
  /// * `inner` - ラップするTokio RwLock
  ///
  /// # 戻り値
  ///
  /// 新しい`TokioRwLockBackend`インスタンス
  pub fn new_with_rwlock(inner: RwLock<T>) -> Self {
    Self { inner }
  }
}

#[async_trait(?Send)]
impl<T> SynchronizedRwBackend<T> for TokioRwLockBackend<T>
where
  T: Send + Sync,
{
  type ReadGuard<'a>
    = RwLockReadGuard<'a, T>
  where
    Self: 'a;
  type WriteGuard<'a>
    = RwLockWriteGuard<'a, T>
  where
    Self: 'a;

  fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      inner: RwLock::new(value),
    }
  }

  async fn read(&self) -> Self::ReadGuard<'_> {
    self.inner.read().await
  }

  async fn write(&self) -> Self::WriteGuard<'_> {
    self.inner.write().await
  }
}

/// Tokioランタイムを使用した排他制御付き共有データ
///
/// `Mutex`による排他的アクセスを提供し、複数のタスク間で安全にデータを共有できます。
pub type Synchronized<T> = CoreSynchronized<TokioMutexBackend<T>, T>;

/// Tokioランタイムを使用した読み書きロック付き共有データ
///
/// `RwLock`による読み取り/書き込みアクセスを提供し、複数の読み取りまたは単一の書き込みを許可します。
pub type SynchronizedRw<T> = CoreSynchronizedRw<TokioRwLockBackend<T>, T>;

#[cfg(test)]
mod tests {
  use super::{Synchronized, SynchronizedRw};

  #[tokio::test]
  async fn synchronized_mutex_read_write() {
    let sync = Synchronized::new(0_u32);

    let read_val = sync.read(|guard| **guard).await;
    assert_eq!(read_val, 0);

    sync
      .write(|guard| {
        **guard = 5;
      })
      .await;

    let result = {
      let guard = sync.lock().await;
      let guard = guard.into_inner();
      *guard
    };

    assert_eq!(result, 5);
  }

  #[tokio::test]
  async fn synchronized_rw_readers_and_writer() {
    let sync = SynchronizedRw::new(vec![1, 2, 3]);

    let sum = sync.read(|guard| guard.iter().copied().sum::<i32>()).await;
    assert_eq!(sum, 6);

    sync
      .write(|guard| {
        guard.push(4);
      })
      .await;

    let len = {
      let guard = sync.read_guard().await;
      guard.len()
    };
    assert_eq!(len, 4);
  }
}
