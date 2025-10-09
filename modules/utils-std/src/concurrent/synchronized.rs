use nexus_utils_core_rs::{
  async_trait, Synchronized as CoreSynchronized, SynchronizedMutexBackend, SynchronizedRw as CoreSynchronizedRw,
  SynchronizedRwBackend,
};
use tokio::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Backend implementation of exclusive control using Tokio Mutex
///
/// Provides exclusive access to shared data.
pub struct TokioMutexBackend<T> {
  inner: Mutex<T>,
}

impl<T> TokioMutexBackend<T> {
  /// Creates a new backend instance from an existing Tokio Mutex.
  ///
  /// # Arguments
  ///
  /// * `inner` - The Tokio Mutex to wrap
  ///
  /// # Returns
  ///
  /// A new `TokioMutexBackend` instance
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
    T: Sized,
  {
    Self {
      inner: Mutex::new(value),
    }
  }

  async fn lock(&self) -> Self::Guard<'_> {
    self.inner.lock().await
  }
}

/// Backend implementation of read-write lock using Tokio RwLock
///
/// Provides multiple read accesses or a single write access.
pub struct TokioRwLockBackend<T> {
  inner: RwLock<T>,
}

impl<T> TokioRwLockBackend<T> {
  /// Creates a new backend instance from an existing Tokio RwLock.
  ///
  /// # Arguments
  ///
  /// * `inner` - The Tokio RwLock to wrap
  ///
  /// # Returns
  ///
  /// A new `TokioRwLockBackend` instance
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
    T: Sized,
  {
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

/// Shared data with exclusive control using Tokio runtime
///
/// Provides exclusive access via `Mutex`, allowing safe data sharing across multiple tasks.
pub type Synchronized<T> = CoreSynchronized<TokioMutexBackend<T>, T>;

/// Shared data with read-write lock using Tokio runtime
///
/// Provides read/write access via `RwLock`, allowing multiple reads or a single write.
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
