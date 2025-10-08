use alloc::boxed::Box;
use alloc::rc::Rc;

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::{Mutex, MutexGuard};
use embassy_sync::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use nexus_utils_core_rs::{
  async_trait, Synchronized as CoreSynchronized, SynchronizedMutexBackend, SynchronizedRw as CoreSynchronizedRw,
  SynchronizedRwBackend,
};

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

  fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      inner: Rc::new(Mutex::new(value)),
    }
  }

  async fn lock(&self) -> Self::Guard<'_> {
    self.inner.lock().await
  }
}

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

  fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      inner: Rc::new(RwLock::new(value)),
    }
  }

  async fn read(&self) -> Self::ReadGuard<'_> {
    self.inner.read().await
  }

  async fn write(&self) -> Self::WriteGuard<'_> {
    self.inner.write().await
  }
}

pub type Synchronized<T> = CoreSynchronized<RcMutexBackend<T>, T>;
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
