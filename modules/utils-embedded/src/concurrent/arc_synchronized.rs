#![cfg(feature = "arc")]

use alloc::boxed::Box;
use alloc::sync::Arc;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, RawMutex};
use embassy_sync::mutex::{Mutex, MutexGuard};
use embassy_sync::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use nexus_utils_core_rs::{
  BoxFuture, Synchronized as CoreSynchronized, SynchronizedMutexBackend, SynchronizedRw as CoreSynchronizedRw,
  SynchronizedRwBackend,
};

#[derive(Clone, Debug)]
pub struct ArcMutexBackend<RM, T>
where
  RM: RawMutex,
{
  inner: Arc<Mutex<RM, T>>,
}

impl<RM, T> SynchronizedMutexBackend<T> for ArcMutexBackend<RM, T>
where
  RM: RawMutex,
  T: Send,
{
  type Guard<'a>
    = MutexGuard<'a, RM, T>
  where
    Self: 'a;
  type LockFuture<'a>
    = BoxFuture<'a, MutexGuard<'a, RM, T>>
  where
    Self: 'a;

  fn new(value: T) -> Self
  where
    T: Sized,
  {
    Self {
      inner: Arc::new(Mutex::new(value)),
    }
  }

  fn lock(&self) -> Self::LockFuture<'_> {
    Box::pin(self.inner.lock())
  }
}

#[derive(Clone, Debug)]
pub struct ArcRwLockBackend<RM, T>
where
  RM: RawMutex,
{
  inner: Arc<RwLock<RM, T>>,
}

impl<RM, T> SynchronizedRwBackend<T> for ArcRwLockBackend<RM, T>
where
  RM: RawMutex,
  T: Send,
{
  type ReadFuture<'a>
    = BoxFuture<'a, RwLockReadGuard<'a, RM, T>>
  where
    Self: 'a;
  type ReadGuard<'a>
    = RwLockReadGuard<'a, RM, T>
  where
    Self: 'a;
  type WriteFuture<'a>
    = BoxFuture<'a, RwLockWriteGuard<'a, RM, T>>
  where
    Self: 'a;
  type WriteGuard<'a>
    = RwLockWriteGuard<'a, RM, T>
  where
    Self: 'a;

  fn new(value: T) -> Self
  where
    T: Sized,
  {
    Self {
      inner: Arc::new(RwLock::new(value)),
    }
  }

  fn read(&self) -> Self::ReadFuture<'_> {
    Box::pin(self.inner.read())
  }

  fn write(&self) -> Self::WriteFuture<'_> {
    Box::pin(self.inner.write())
  }
}

pub type ArcSynchronized<T, RM> = CoreSynchronized<ArcMutexBackend<RM, T>, T>;
pub type ArcSynchronizedRw<T, RM> = CoreSynchronizedRw<ArcRwLockBackend<RM, T>, T>;

pub type ArcLocalSynchronized<T> = ArcSynchronized<T, CriticalSectionRawMutex>;
pub type ArcLocalSynchronizedRw<T> = ArcSynchronizedRw<T, CriticalSectionRawMutex>;
pub type ArcCsSynchronized<T> = ArcLocalSynchronized<T>;
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
