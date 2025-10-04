#![allow(dead_code)]

use alloc::boxed::Box;
use core::future::Future;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::time::Duration;

use embassy_futures::yield_now;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::{Mutex, MutexGuard};
use embassy_sync::rwlock::{RwLock as EmbassyRwLock, RwLockReadGuard, RwLockWriteGuard};
use embassy_sync::signal::Signal;
use embassy_time::{Duration as EmbassyDuration, Timer as EmbassyTimer};
use nexus_utils_core_rs::async_primitives::{AsyncMutex, AsyncNotify, AsyncRwLock, AsyncYield, Timer};

/// Embassy 実装の Timer ラッパ。
#[derive(Default, Clone)]
pub struct EmbassyTimerWrapper;

impl Timer for EmbassyTimerWrapper {
  fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    Box::pin(EmbassyTimer::after(to_embassy_duration(duration)))
  }
}

/// Embassy の yield 実装。
#[derive(Default, Clone)]
pub struct EmbassyYield;

impl AsyncYield for EmbassyYield {
  fn yield_now(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    Box::pin(async move {
      yield_now().await;
    })
  }
}

/// Embassy Notify ラッパ。
pub struct EmbassyNotify {
  inner: Signal<CriticalSectionRawMutex, ()>,
}

impl EmbassyNotify {
  pub const fn new() -> Self {
    Self { inner: Signal::new() }
  }
}

impl Default for EmbassyNotify {
  fn default() -> Self {
    Self::new()
  }
}

impl AsyncNotify for EmbassyNotify {
  fn notify_one(&self) {
    self.inner.signal(());
  }

  fn notify_waiters(&self) {
    self.inner.signal(());
  }

  fn wait(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    let inner = &self.inner;
    Box::pin(async move {
      inner.wait().await;
    })
  }
}

/// TODO: Mutex / RwLock 実装は今後追加する。
pub struct EmbassyMutex<T> {
  inner: Mutex<CriticalSectionRawMutex, T>,
}

impl<T> EmbassyMutex<T> {
  pub fn new(value: T) -> Self {
    Self {
      inner: Mutex::new(value),
    }
  }
}

pub struct EmbassyMutexGuard<'a, T> {
  guard: MutexGuard<'a, CriticalSectionRawMutex, T>,
}

impl<'a, T> Deref for EmbassyMutexGuard<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &*self.guard
  }
}

impl<'a, T> DerefMut for EmbassyMutexGuard<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut *self.guard
  }
}

impl<T: Send + 'static> AsyncMutex<T> for EmbassyMutex<T> {
  type Guard<'a>
    = EmbassyMutexGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn lock(&self) -> Pin<Box<dyn Future<Output = Self::Guard<'_>> + Send + '_>> {
    let inner = &self.inner;
    Box::pin(async move {
      let guard = inner.lock().await;
      EmbassyMutexGuard { guard }
    })
  }
}

/// Embassy ベースの RwLock 実装。
pub struct EmbeddedRwLock<T> {
  inner: EmbassyRwLock<CriticalSectionRawMutex, T>,
}

impl<T> EmbeddedRwLock<T> {
  pub const fn new(value: T) -> Self {
    Self {
      inner: EmbassyRwLock::new(value),
    }
  }
}

pub struct EmbeddedRwReadGuard<'a, T> {
  guard: RwLockReadGuard<'a, CriticalSectionRawMutex, T>,
}

impl<'a, T> Deref for EmbeddedRwReadGuard<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &*self.guard
  }
}

pub struct EmbeddedRwWriteGuard<'a, T> {
  guard: RwLockWriteGuard<'a, CriticalSectionRawMutex, T>,
}

impl<'a, T> Deref for EmbeddedRwWriteGuard<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &*self.guard
  }
}

impl<'a, T> DerefMut for EmbeddedRwWriteGuard<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut *self.guard
  }
}

impl<T: Send + Sync + 'static> AsyncRwLock<T> for EmbeddedRwLock<T> {
  type ReadGuard<'a>
    = EmbeddedRwReadGuard<'a, T>
  where
    Self: 'a,
    T: 'a;
  type WriteGuard<'a>
    = EmbeddedRwWriteGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn read(&self) -> Pin<Box<dyn Future<Output = Self::ReadGuard<'_>> + Send + '_>> {
    Box::pin(async move {
      let guard = self.inner.read().await;
      EmbeddedRwReadGuard { guard }
    })
  }

  fn write(&self) -> Pin<Box<dyn Future<Output = Self::WriteGuard<'_>> + Send + '_>> {
    Box::pin(async move {
      let guard = self.inner.write().await;
      EmbeddedRwWriteGuard { guard }
    })
  }
}

fn to_embassy_duration(duration: Duration) -> EmbassyDuration {
  let secs = EmbassyDuration::from_secs(duration.as_secs());
  let nanos = EmbassyDuration::from_nanos(duration.subsec_nanos() as u64);
  secs.checked_add(nanos).unwrap_or(EmbassyDuration::MAX)
}

#[cfg(test)]
mod tests {
  use super::*;
  use alloc::boxed::Box;
  use core::future::Future;
  use core::pin::Pin;
  use core::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
  use critical_section::Impl;
  use static_assertions::assert_impl_all;
  use std::sync::atomic::{AtomicBool, Ordering};

  struct StdCriticalSection;

  critical_section::set_impl!(StdCriticalSection);

  static CS_LOCK: AtomicBool = AtomicBool::new(false);

  unsafe impl Impl for StdCriticalSection {
    unsafe fn acquire() -> critical_section::RawRestoreState {
      while CS_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
      {
        core::hint::spin_loop();
      }
      ()
    }

    unsafe fn release(_: critical_section::RawRestoreState) {
      CS_LOCK.store(false, Ordering::Release);
    }
  }

  fn dummy_waker() -> Waker {
    fn noop_clone(_: *const ()) -> RawWaker {
      RawWaker::new(core::ptr::null(), &VTABLE)
    }
    fn noop(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(noop_clone, noop, noop, noop);

    let raw = RawWaker::new(core::ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(raw) }
  }

  fn poll_once<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    let waker = dummy_waker();
    let mut cx = Context::from_waker(&waker);
    future.poll(&mut cx)
  }

  #[test]
  fn embedded_rwlock_is_send_sync() {
    assert_impl_all!(EmbeddedRwLock<u32>: Send, Sync);
  }

  #[test]
  fn multiple_read_guards_can_coexist() {
    let lock = EmbeddedRwLock::new(10u32);

    let mut read1 = Box::pin(lock.read());
    let guard1 = match poll_once(read1.as_mut()) {
      Poll::Ready(g) => g,
      Poll::Pending => panic!("first read should not block"),
    };

    let mut read2 = Box::pin(lock.read());
    let guard2 = match poll_once(read2.as_mut()) {
      Poll::Ready(g) => g,
      Poll::Pending => panic!("second read should not block"),
    };

    assert_eq!(*guard1, 10);
    assert_eq!(*guard2, 10);
  }

  #[test]
  fn write_guard_waits_for_readers() {
    let lock = EmbeddedRwLock::new(0u32);

    let mut read_future = Box::pin(lock.read());
    let read_guard = match poll_once(read_future.as_mut()) {
      Poll::Ready(g) => g,
      Poll::Pending => panic!("initial read should be ready"),
    };

    let mut write_future = Box::pin(lock.write());
    assert!(matches!(poll_once(write_future.as_mut()), Poll::Pending));

    drop(read_guard);

    let write_guard = match poll_once(write_future.as_mut()) {
      Poll::Ready(g) => g,
      Poll::Pending => panic!("write should complete after readers drop"),
    };

    drop(write_guard);
  }
}
