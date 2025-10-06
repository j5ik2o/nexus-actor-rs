#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use core::future::Future;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::time::Duration;

/// 抽象的な非同期 Mutex インターフェース。
/// 実装は待機可能な `lock` を提供し、ガードがドロップされるまで排他アクセスを保証します。
///
/// # Examples
/// ```ignore
/// use nexus_utils_core_rs::async_primitives::AsyncMutex;
/// use std::pin::Pin;
/// use std::future::Future;
///
/// struct InlineMutex<T>(tokio::sync::Mutex<T>);
///
/// impl<T: Send + 'static> AsyncMutex<T> for InlineMutex<T> {
///     type Guard<'a> = tokio::sync::MutexGuard<'a, T> where Self: 'a, T: 'a;
///
///     fn lock(&self) -> Pin<Box<dyn Future<Output = Self::Guard<'_>> + Send + '_>> {
///         Box::pin(self.0.lock())
///     }
/// }
/// ```
pub trait AsyncMutex<T>: Send + Sync {
  type Guard<'a>: Deref<Target = T> + DerefMut + Send + 'a
  where
    Self: 'a,
    T: 'a;

  fn lock(&self) -> Pin<Box<dyn Future<Output = Self::Guard<'_>> + Send + '_>>;
}

/// 複数リーダー・単独ライターを想定した非同期 RwLock 抽象。
/// 実装はリーダー・ライターそれぞれの Future を提供します。
///
/// # Examples
/// ```ignore
/// use nexus_utils_core_rs::async_primitives::AsyncRwLock;
/// use std::pin::Pin;
/// use std::future::Future;
///
/// struct InlineRwLock<T>(tokio::sync::RwLock<T>);
///
/// impl<T: Send + Sync + 'static> AsyncRwLock<T> for InlineRwLock<T> {
///     type ReadGuard<'a> = tokio::sync::RwLockReadGuard<'a, T> where Self: 'a, T: 'a;
///     type WriteGuard<'a> = tokio::sync::RwLockWriteGuard<'a, T> where Self: 'a, T: 'a;
///
///     fn read(&self) -> Pin<Box<dyn Future<Output = Self::ReadGuard<'_>> + Send + '_>> {
///         Box::pin(self.0.read())
///     }
///
///     fn write(&self) -> Pin<Box<dyn Future<Output = Self::WriteGuard<'_>> + Send + '_>> {
///         Box::pin(self.0.write())
///     }
/// }
/// ```
pub trait AsyncRwLock<T>: Send + Sync {
  type ReadGuard<'a>: Deref<Target = T> + Send + 'a
  where
    Self: 'a,
    T: 'a;

  type WriteGuard<'a>: Deref<Target = T> + DerefMut + Send + 'a
  where
    Self: 'a,
    T: 'a;

  fn read(&self) -> Pin<Box<dyn Future<Output = Self::ReadGuard<'_>> + Send + '_>>;
  fn write(&self) -> Pin<Box<dyn Future<Output = Self::WriteGuard<'_>> + Send + '_>>;
}

/// 待機者を通知するための抽象。`wait` は通知されるまで待機する Future を返します。
///
/// # Examples
/// ```ignore
/// # use nexus_utils_core_rs::async_primitives::AsyncNotify;
/// # use std::pin::Pin;
/// # use std::future::Future;
/// struct InlineNotify(tokio::sync::Notify);
///
/// impl AsyncNotify for InlineNotify {
///     fn notify_one(&self) { self.0.notify_one(); }
///     fn notify_waiters(&self) { self.0.notify_waiters(); }
///     fn wait(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
///         Box::pin(self.0.notified())
///     }
/// }
/// ```
pub trait AsyncNotify: Send + Sync {
  fn notify_one(&self);
  fn notify_waiters(&self);
  fn wait(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

/// スリープ機構を抽象化し、任意のランタイムで時間待ちを表現します。
///
/// # Examples
/// ```ignore
/// # use nexus_utils_core_rs::async_primitives::Timer;
/// # use std::pin::Pin;
/// # use std::future::Future;
/// struct InlineTimer;
///
/// impl Timer for InlineTimer {
///     fn sleep(&self, duration: core::time::Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
///         Box::pin(tokio::time::sleep(duration))
///     }
/// }
/// ```
pub trait Timer: Send + Sync {
  fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

pub trait AsyncYield: Send + Sync {
  fn yield_now(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

#[cfg(test)]
mod tests {
  use super::*;
  use alloc::sync::Arc;
  use core::cell::UnsafeCell;
  use core::future::Future;
  use core::ops::{Deref, DerefMut};
  use core::pin::Pin;
  use core::sync::atomic::{AtomicBool, Ordering};
  use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

  fn dummy_waker() -> Waker {
    fn noop_clone(_: *const ()) -> RawWaker {
      RawWaker::new(core::ptr::null(), &VTABLE)
    }
    fn noop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);

    let raw = RawWaker::new(core::ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(raw) }
  }

  fn poll_ready<'a, T>(future: &mut Pin<Box<dyn Future<Output = T> + Send + 'a>>) -> T {
    let waker = dummy_waker();
    let mut cx = Context::from_waker(&waker);
    match future.as_mut().poll(&mut cx) {
      Poll::Ready(value) => value,
      Poll::Pending => panic!("future should not pend in dummy implementations"),
    }
  }

  struct DummyMutex<T> {
    locked: AtomicBool,
    value: UnsafeCell<T>,
  }

  unsafe impl<T: Send> Send for DummyMutex<T> {}
  unsafe impl<T: Send> Sync for DummyMutex<T> {}

  impl<T> DummyMutex<T> {
    fn new(value: T) -> Self {
      Self {
        locked: AtomicBool::new(false),
        value: UnsafeCell::new(value),
      }
    }
  }

  struct DummyMutexGuard<'a, T> {
    mutex: &'a DummyMutex<T>,
  }

  impl<'a, T> Deref for DummyMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
      unsafe { &*self.mutex.value.get() }
    }
  }

  impl<'a, T> DerefMut for DummyMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
      unsafe { &mut *self.mutex.value.get() }
    }
  }

  impl<'a, T> Drop for DummyMutexGuard<'a, T> {
    fn drop(&mut self) {
      self.mutex.locked.store(false, Ordering::SeqCst);
    }
  }

  impl<T: Send> AsyncMutex<T> for DummyMutex<T> {
    type Guard<'a>
      = DummyMutexGuard<'a, T>
    where
      Self: 'a,
      T: 'a;

    fn lock(&self) -> Pin<Box<dyn Future<Output = Self::Guard<'_>> + Send + '_>> {
      Box::pin(async move {
        let was_locked = self.locked.swap(true, Ordering::SeqCst);
        assert!(!was_locked, "DummyMutex does not support contention");
        DummyMutexGuard { mutex: self }
      })
    }
  }

  struct DummyRwLock<T> {
    writer: AtomicBool,
    value: UnsafeCell<T>,
  }

  unsafe impl<T: Send + Sync> Send for DummyRwLock<T> {}
  unsafe impl<T: Send + Sync> Sync for DummyRwLock<T> {}

  impl<T> DummyRwLock<T> {
    fn new(value: T) -> Self {
      Self {
        writer: AtomicBool::new(false),
        value: UnsafeCell::new(value),
      }
    }
  }

  struct DummyReadGuard<'a, T> {
    lock: &'a DummyRwLock<T>,
  }

  impl<'a, T> Deref for DummyReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
      unsafe { &*self.lock.value.get() }
    }
  }

  struct DummyWriteGuard<'a, T> {
    lock: &'a DummyRwLock<T>,
  }

  impl<'a, T> Deref for DummyWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
      unsafe { &*self.lock.value.get() }
    }
  }

  impl<'a, T> DerefMut for DummyWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
      unsafe { &mut *self.lock.value.get() }
    }
  }

  impl<'a, T> Drop for DummyWriteGuard<'a, T> {
    fn drop(&mut self) {
      self.lock.writer.store(false, Ordering::SeqCst);
    }
  }

  impl<T: Send + Sync> AsyncRwLock<T> for DummyRwLock<T> {
    type ReadGuard<'a>
      = DummyReadGuard<'a, T>
    where
      Self: 'a,
      T: 'a;
    type WriteGuard<'a>
      = DummyWriteGuard<'a, T>
    where
      Self: 'a,
      T: 'a;

    fn read(&self) -> Pin<Box<dyn Future<Output = Self::ReadGuard<'_>> + Send + '_>> {
      Box::pin(async move { DummyReadGuard { lock: self } })
    }

    fn write(&self) -> Pin<Box<dyn Future<Output = Self::WriteGuard<'_>> + Send + '_>> {
      Box::pin(async move {
        let was_writer = self.writer.swap(true, Ordering::SeqCst);
        assert!(!was_writer, "DummyRwLock does not support contention");
        DummyWriteGuard { lock: self }
      })
    }
  }

  struct DummyNotify {
    flag: AtomicBool,
  }

  impl DummyNotify {
    fn new() -> Self {
      Self {
        flag: AtomicBool::new(false),
      }
    }
  }

  impl AsyncNotify for DummyNotify {
    fn notify_one(&self) {
      self.flag.store(true, Ordering::SeqCst);
    }

    fn notify_waiters(&self) {
      self.notify_one();
    }

    fn wait(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
      Box::pin(async move { while !self.flag.swap(false, Ordering::SeqCst) {} })
    }
  }

  #[derive(Default)]
  struct DummyTimer;

  impl Timer for DummyTimer {
    fn sleep(&self, _duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
      Box::pin(async {})
    }
  }

  #[test]
  fn async_mutex_provides_exclusive_access() {
    let mutex = DummyMutex::new(1u32);
    {
      let mut fut = mutex.lock();
      let mut guard = poll_ready(&mut fut);
      *guard += 1;
    }
    {
      let mut fut = mutex.lock();
      let guard = poll_ready(&mut fut);
      assert_eq!(*guard, 2);
    }
  }

  #[test]
  fn async_rwlock_allows_read_and_write() {
    let lock = DummyRwLock::new(Arc::new(5usize));
    {
      let mut fut = lock.read();
      let guard = poll_ready(&mut fut);
      assert_eq!(**guard, 5);
    }
    {
      let mut fut = lock.write();
      let mut guard = poll_ready(&mut fut);
      *Arc::get_mut(&mut guard).expect("no other refs") = 8;
    }
    {
      let mut fut = lock.read();
      let guard = poll_ready(&mut fut);
      assert_eq!(**guard, 8);
    }
  }

  #[test]
  fn async_notify_wait_clears_flag() {
    let notify = DummyNotify::new();
    notify.notify_one();
    let mut fut = notify.wait();
    poll_ready(&mut fut);
    assert!(!notify.flag.load(Ordering::SeqCst));
  }

  #[test]
  fn timer_sleep_completes_immediately() {
    let timer = DummyTimer::default();
    let mut fut = timer.sleep(Duration::from_millis(1));
    poll_ready(&mut fut);
  }
}
