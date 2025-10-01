#![cfg(feature = "alloc")]

use alloc::boxed::Box;
use core::future::Future;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::time::Duration;

/// 非同期 Mutex の抽象化。
pub trait AsyncMutex<T>: Send + Sync {
  type Guard<'a>: Deref<Target = T> + DerefMut + Send + 'a
  where
    Self: 'a,
    T: 'a;

  fn lock(&self) -> Pin<Box<dyn Future<Output = Self::Guard<'_>> + Send + '_>>;
}

/// 非同期 RwLock の抽象化。
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

/// 通知プリミティブの抽象化。
pub trait AsyncNotify: Send + Sync {
  fn notify_one(&self);
  fn notify_waiters(&self);
  fn wait(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

/// スリープ機能の抽象化。
pub trait Timer: Send + Sync {
  fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}
