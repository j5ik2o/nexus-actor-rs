use futures::future::BoxFuture;
use std::future::Future;
use tokio::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug)]
pub struct Synchronized<T: ?Sized> {
  inner: Mutex<T>,
}

impl<T: ?Sized> Synchronized<T> {
  pub fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      inner: Mutex::new(value),
    }
  }

  pub async fn read<R>(&self, f: impl FnOnce(&MutexGuard<T>) -> R) -> R {
    let guard = self.inner.lock().await;
    f(&guard)
  }

  pub async fn write<R>(&self, f: impl FnOnce(&mut MutexGuard<T>) -> R) -> R {
    let mut guard = self.inner.lock().await;
    f(&mut guard)
  }

  pub async fn read_async<F, Fut, R>(&self, f: F) -> R
  where
    F: FnOnce(&MutexGuard<T>) -> Fut + Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: Send + 'static, {
    let guard = self.inner.lock().await;
    f(&guard).await
  }

  pub async fn read_async_boxed<F, R>(&self, f: F) -> R
  where
    F: for<'a> FnOnce(&'a MutexGuard<T>) -> BoxFuture<'a, R> + Send + 'static,
    R: Send + 'static, {
    let guard = self.inner.lock().await;
    f(&guard).await
  }

  pub async fn write_async<F, Fut, R>(&self, f: F) -> R
  where
    F: FnOnce(&mut MutexGuard<T>) -> Fut + Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: Send + 'static, {
    let mut guard = self.inner.lock().await;
    f(&mut guard).await
  }

  pub async fn write_async_boxed<F, R>(&self, f: F) -> R
  where
    F: for<'a> FnOnce(&'a mut MutexGuard<T>) -> BoxFuture<'a, R> + Send + 'static,
    R: Send + 'static, {
    let mut guard = self.inner.lock().await;
    f(&mut guard).await
  }
}

impl<T: Default> Default for Synchronized<T> {
  fn default() -> Self {
    Self::new(T::default())
  }
}

impl<T> From<T> for Synchronized<T> {
  fn from(value: T) -> Self {
    Self::new(value)
  }
}

#[derive(Debug)]
pub struct SynchronizedRw<T: ?Sized> {
  inner: RwLock<T>,
}

impl<T: ?Sized> SynchronizedRw<T> {
  pub fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      inner: RwLock::new(value),
    }
  }

  pub async fn read<R>(&self, f: impl FnOnce(&RwLockReadGuard<T>) -> R) -> R {
    let guard = self.inner.read().await;
    f(&guard)
  }

  pub async fn write<R>(&self, f: impl FnOnce(&mut RwLockWriteGuard<T>) -> R) -> R {
    let mut guard = self.inner.write().await;
    f(&mut guard)
  }

  pub async fn read_async<F, Fut, R>(&self, f: F) -> R
  where
    F: FnOnce(&RwLockReadGuard<T>) -> Fut + Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: Send + 'static, {
    let guard = self.inner.read().await;
    f(&guard).await
  }

  pub async fn read_async_boxed<F, R>(&self, f: F) -> R
  where
    F: for<'a> FnOnce(&'a RwLockReadGuard<T>) -> BoxFuture<'a, R> + Send + 'static,
    R: Send + 'static, {
    let guard = self.inner.read().await;
    f(&guard).await
  }

  pub async fn write_async<F, Fut, R>(&self, f: F) -> R
  where
    F: FnOnce(&mut RwLockWriteGuard<T>) -> Fut + Send + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: Send + 'static, {
    let mut guard = self.inner.write().await;
    f(&mut guard).await
  }

  pub async fn write_async_boxed<F, R>(&self, f: F) -> R
  where
    F: for<'a> FnOnce(&'a mut RwLockWriteGuard<T>) -> BoxFuture<'a, R> + Send + 'static,
    R: Send + 'static, {
    let mut guard = self.inner.write().await;
    f(&mut guard).await
  }
}

impl<T: Default> Default for SynchronizedRw<T> {
  fn default() -> Self {
    Self::new(T::default())
  }
}

impl<T> From<T> for SynchronizedRw<T> {
  fn from(value: T) -> Self {
    Self::new(value)
  }
}
