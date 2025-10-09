#![cfg(feature = "arc")]

use alloc::boxed::Box;
use alloc::sync::Arc;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, RawMutex};
use embassy_sync::mutex::{Mutex, MutexGuard};
use embassy_sync::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use nexus_utils_core_rs::{
  async_trait, Synchronized as CoreSynchronized, SynchronizedMutexBackend, SynchronizedRw as CoreSynchronizedRw,
  SynchronizedRwBackend,
};

/// Backend implementation for mutex-based synchronization using `Arc`
///
/// Provides exclusive-access synchronization using embassy-sync's `Mutex` with
/// `Arc` for thread-safe reference counting.
///
/// # Type Parameters
///
/// * `RM` - Raw mutex type from embassy-sync
/// * `T` - The value type being synchronized
#[derive(Clone, Debug)]
pub struct ArcMutexBackend<RM, T>
where
  RM: RawMutex, {
  inner: Arc<Mutex<RM, T>>,
}

#[async_trait(?Send)]
impl<RM, T> SynchronizedMutexBackend<T> for ArcMutexBackend<RM, T>
where
  RM: RawMutex,
  T: Send,
{
  type Guard<'a>
    = MutexGuard<'a, RM, T>
  where
    Self: 'a;

  fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      inner: Arc::new(Mutex::new(value)),
    }
  }

  async fn lock(&self) -> Self::Guard<'_> {
    self.inner.lock().await
  }
}

/// Backend implementation for read-write lock synchronization using `Arc`
///
/// Provides concurrent read access with exclusive write access using embassy-sync's
/// `RwLock` with `Arc` for thread-safe reference counting.
///
/// # Type Parameters
///
/// * `RM` - Raw mutex type from embassy-sync
/// * `T` - The value type being synchronized
#[derive(Clone, Debug)]
pub struct ArcRwLockBackend<RM, T>
where
  RM: RawMutex, {
  inner: Arc<RwLock<RM, T>>,
}

#[async_trait(?Send)]
impl<RM, T> SynchronizedRwBackend<T> for ArcRwLockBackend<RM, T>
where
  RM: RawMutex,
  T: Send,
{
  type ReadGuard<'a>
    = RwLockReadGuard<'a, RM, T>
  where
    Self: 'a;
  type WriteGuard<'a>
    = RwLockWriteGuard<'a, RM, T>
  where
    Self: 'a;

  fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      inner: Arc::new(RwLock::new(value)),
    }
  }

  async fn read(&self) -> Self::ReadGuard<'_> {
    self.inner.read().await
  }

  async fn write(&self) -> Self::WriteGuard<'_> {
    self.inner.write().await
  }
}

/// Type alias for `Arc`-based mutex synchronization
///
/// Provides exclusive-access synchronization with configurable mutex backend.
pub type ArcSynchronized<T, RM> = CoreSynchronized<ArcMutexBackend<RM, T>, T>;

/// Type alias for `Arc`-based read-write lock synchronization
///
/// Provides concurrent reads with exclusive writes using configurable mutex backend.
pub type ArcSynchronizedRw<T, RM> = CoreSynchronizedRw<ArcRwLockBackend<RM, T>, T>;

/// Type alias for `ArcSynchronized` using `CriticalSectionRawMutex`
///
/// Provides interrupt-safe critical section protection for embedded contexts.
pub type ArcLocalSynchronized<T> = ArcSynchronized<T, CriticalSectionRawMutex>;

/// Type alias for `ArcSynchronizedRw` using `CriticalSectionRawMutex`
///
/// Provides interrupt-safe read-write lock for embedded contexts.
pub type ArcLocalSynchronizedRw<T> = ArcSynchronizedRw<T, CriticalSectionRawMutex>;

/// Alias for `ArcLocalSynchronized` for consistency
///
/// Uses critical section mutex backend.
pub type ArcCsSynchronized<T> = ArcLocalSynchronized<T>;

/// Alias for `ArcLocalSynchronizedRw` for consistency
///
/// Uses critical section rwlock backend.
pub type ArcCsSynchronizedRw<T> = ArcLocalSynchronizedRw<T>;

#[cfg(all(test, feature = "std"))]
mod tests {
  use alloc::vec;

  use super::{ArcLocalSynchronized, ArcLocalSynchronizedRw};
  use futures::executor::block_on;

  #[test]
  fn arc_mutex_backend_behaviour() {
    block_on(async {
      let sync = ArcLocalSynchronized::new(10);
      let value = sync.read(|guard| **guard).await;
      assert_eq!(value, 10);

      sync
        .write(|guard| {
          **guard = 20;
        })
        .await;

      let updated = {
        let guard = sync.lock().await;
        let guard = guard.into_inner();
        *guard
      };
      assert_eq!(updated, 20);
    });
  }

  #[test]
  fn arc_rw_backend_behaviour() {
    block_on(async {
      let sync = ArcLocalSynchronizedRw::new(vec![1, 2]);
      let len = sync.read(|guard| guard.len()).await;
      assert_eq!(len, 2);

      sync
        .write(|guard| {
          guard.push(3);
        })
        .await;

      let sum = {
        let guard = sync.read_guard().await;
        guard.iter().copied().sum::<i32>()
      };
      assert_eq!(sum, 6);
    });
  }
}
