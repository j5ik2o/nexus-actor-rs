use core::future::Future;
use core::pin::Pin;
use core::time::Duration;
use std::boxed::Box;

use nexus_actor_core_rs::runtime::{AsyncMutex, AsyncNotify, AsyncRwLock, Timer};
use tokio::sync::{Mutex, MutexGuard, Notify, RwLock, RwLockReadGuard, RwLockWriteGuard};

pub struct TokioMutex<T>(Mutex<T>);

impl<T> TokioMutex<T> {
  pub fn new(value: T) -> Self {
    Self(Mutex::new(value))
  }

  pub fn into_inner(self) -> Mutex<T> {
    self.0
  }
}

impl<T: Send + 'static> AsyncMutex<T> for TokioMutex<T> {
  type Guard<'a>
    = MutexGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn lock(&self) -> Pin<Box<dyn Future<Output = Self::Guard<'_>> + Send + '_>> {
    Box::pin(self.0.lock())
  }
}

pub struct TokioRwLock<T>(RwLock<T>);

impl<T> TokioRwLock<T> {
  pub fn new(value: T) -> Self {
    Self(RwLock::new(value))
  }
}

impl<T: Send + Sync + 'static> AsyncRwLock<T> for TokioRwLock<T> {
  type ReadGuard<'a>
    = RwLockReadGuard<'a, T>
  where
    Self: 'a,
    T: 'a;
  type WriteGuard<'a>
    = RwLockWriteGuard<'a, T>
  where
    Self: 'a,
    T: 'a;

  fn read(&self) -> Pin<Box<dyn Future<Output = Self::ReadGuard<'_>> + Send + '_>> {
    Box::pin(self.0.read())
  }

  fn write(&self) -> Pin<Box<dyn Future<Output = Self::WriteGuard<'_>> + Send + '_>> {
    Box::pin(self.0.write())
  }
}

pub struct TokioNotify(Notify);

impl TokioNotify {
  pub fn new() -> Self {
    Self(Notify::new())
  }

  pub fn into_inner(self) -> Notify {
    self.0
  }
}

impl AsyncNotify for TokioNotify {
  fn notify_one(&self) {
    self.0.notify_one();
  }

  fn notify_waiters(&self) {
    self.0.notify_waiters();
  }

  fn wait(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    Box::pin(self.0.notified())
  }
}

pub struct TokioTimer;

impl Timer for TokioTimer {
  fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
    Box::pin(tokio::time::sleep(duration))
  }
}
