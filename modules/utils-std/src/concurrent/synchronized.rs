use nexus_utils_core_rs::{
  BoxFuture, Synchronized as CoreSynchronized, SynchronizedMutexBackend, SynchronizedRw as CoreSynchronizedRw,
  SynchronizedRwBackend,
};
use tokio::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct TokioMutexBackend<T> {
  inner: Mutex<T>,
}

impl<T> TokioMutexBackend<T> {
  pub fn new_with_mutex(inner: Mutex<T>) -> Self {
    Self { inner }
  }
}

impl<T> SynchronizedMutexBackend<T> for TokioMutexBackend<T>
where
  T: Send,
{
  type Guard<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a;
  type LockFuture<'a>
    = BoxFuture<'a, MutexGuard<'a, T>>
  where
    Self: 'a;

  fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      inner: Mutex::new(value),
    }
  }

  fn lock(&self) -> Self::LockFuture<'_> {
    Box::pin(self.inner.lock())
  }
}

pub struct TokioRwLockBackend<T> {
  inner: RwLock<T>,
}

impl<T> TokioRwLockBackend<T> {
  pub fn new_with_rwlock(inner: RwLock<T>) -> Self {
    Self { inner }
  }
}

impl<T> SynchronizedRwBackend<T> for TokioRwLockBackend<T>
where
  T: Send + Sync,
{
  type ReadFuture<'a>
    = BoxFuture<'a, RwLockReadGuard<'a, T>>
  where
    Self: 'a;
  type ReadGuard<'a>
    = RwLockReadGuard<'a, T>
  where
    Self: 'a;
  type WriteFuture<'a>
    = BoxFuture<'a, RwLockWriteGuard<'a, T>>
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

  fn read(&self) -> Self::ReadFuture<'_> {
    Box::pin(self.inner.read())
  }

  fn write(&self) -> Self::WriteFuture<'_> {
    Box::pin(self.inner.write())
  }
}

pub type Synchronized<T> = CoreSynchronized<TokioMutexBackend<T>, T>;
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
