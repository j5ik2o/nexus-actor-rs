#![allow(dead_code)]

use alloc::boxed::Box;
use core::future::Future;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::time::Duration;

use embassy_futures::yield_now;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::mutex::{Mutex, MutexGuard};
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

/// 仮の RwLock 実装。今後 `AsyncRwLock` の要件を満たすよう改善する。
pub struct EmbassyRwLock<T> {
  inner: EmbassyMutex<T>,
}

impl<T> EmbassyRwLock<T> {
  pub fn new(value: T) -> Self {
    Self {
      inner: EmbassyMutex::new(value),
    }
  }
}

pub struct EmbassyRwReadGuard<'a, T>(EmbassyMutexGuard<'a, T>);

impl<'a, T> Deref for EmbassyRwReadGuard<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &*self.0
  }
}

pub struct EmbassyRwWriteGuard<'a, T>(EmbassyMutexGuard<'a, T>);

impl<'a, T> Deref for EmbassyRwWriteGuard<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &*self.0
  }
}

impl<'a, T> DerefMut for EmbassyRwWriteGuard<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut *self.0
  }
}

impl<T: Send + Sync + 'static> AsyncRwLock<T> for EmbassyRwLock<T> {
  type ReadGuard<'a>
    = EmbassyRwReadGuard<'a, T>
  where
    Self: 'a,
    T: 'a;
  type WriteGuard<'a>
    = EmbassyRwWriteGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn read(&self) -> Pin<Box<dyn Future<Output = Self::ReadGuard<'_>> + Send + '_>> {
    let inner = &self.inner;
    Box::pin(async move {
      let guard = inner.lock().await;
      EmbassyRwReadGuard(guard)
    })
  }

  fn write(&self) -> Pin<Box<dyn Future<Output = Self::WriteGuard<'_>> + Send + '_>> {
    let inner = &self.inner;
    Box::pin(async move {
      let guard = inner.lock().await;
      EmbassyRwWriteGuard(guard)
    })
  }
}

fn to_embassy_duration(duration: Duration) -> EmbassyDuration {
  let secs = EmbassyDuration::from_secs(duration.as_secs());
  let nanos = EmbassyDuration::from_nanos(duration.subsec_nanos() as u64);
  secs.checked_add(nanos).unwrap_or(EmbassyDuration::MAX)
}
