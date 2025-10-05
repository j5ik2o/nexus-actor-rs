use alloc::boxed::Box;
use core::future::Future;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;

/// Heap-allocated future type alias used by synchronized helpers.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

#[derive(Debug)]
pub struct GuardHandle<G> {
  guard: G,
}

impl<G> GuardHandle<G> {
  pub fn new(guard: G) -> Self {
    Self { guard }
  }

  pub fn into_inner(self) -> G {
    self.guard
  }
}

impl<G> Deref for GuardHandle<G>
where
  G: Deref,
{
  type Target = G::Target;

  fn deref(&self) -> &Self::Target {
    &self.guard
  }
}

impl<G> DerefMut for GuardHandle<G>
where
  G: DerefMut,
{
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.guard
  }
}

/// Backend trait for async mutex-like primitives.
pub trait SynchronizedMutexBackend<T: ?Sized> {
  type Guard<'a>: Deref<Target = T> + DerefMut + 'a
  where
    Self: 'a;

  type LockFuture<'a>: Future<Output = Self::Guard<'a>> + 'a
  where
    Self: 'a;

  fn new(value: T) -> Self
  where
    T: Sized;

  fn lock(&self) -> Self::LockFuture<'_>;
}

/// Backend trait for async read/write lock primitives.
pub trait SynchronizedRwBackend<T: ?Sized> {
  type ReadGuard<'a>: Deref<Target = T> + 'a
  where
    Self: 'a;

  type WriteGuard<'a>: Deref<Target = T> + DerefMut + 'a
  where
    Self: 'a;

  type ReadFuture<'a>: Future<Output = Self::ReadGuard<'a>> + 'a
  where
    Self: 'a;

  type WriteFuture<'a>: Future<Output = Self::WriteGuard<'a>> + 'a
  where
    Self: 'a;

  fn new(value: T) -> Self
  where
    T: Sized;

  fn read(&self) -> Self::ReadFuture<'_>;

  fn write(&self) -> Self::WriteFuture<'_>;
}

#[derive(Debug)]
pub struct Synchronized<B, T: ?Sized>
where
  B: SynchronizedMutexBackend<T>, {
  backend: B,
  _marker: PhantomData<T>,
}

impl<B, T> Synchronized<B, T>
where
  T: ?Sized,
  B: SynchronizedMutexBackend<T>,
{
  pub fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      backend: B::new(value),
      _marker: PhantomData,
    }
  }

  pub fn from_backend(backend: B) -> Self {
    Self {
      backend,
      _marker: PhantomData,
    }
  }

  pub fn backend(&self) -> &B {
    &self.backend
  }

  pub async fn read<R>(&self, f: impl FnOnce(&B::Guard<'_>) -> R) -> R {
    let guard = self.backend.lock().await;
    f(&guard)
  }

  pub async fn write<R>(&self, f: impl FnOnce(&mut B::Guard<'_>) -> R) -> R {
    let mut guard = self.backend.lock().await;
    f(&mut guard)
  }

  pub async fn lock(&self) -> GuardHandle<B::Guard<'_>> {
    let guard = self.backend.lock().await;
    GuardHandle::new(guard)
  }
}

impl<B, T> Default for Synchronized<B, T>
where
  T: Default,
  B: SynchronizedMutexBackend<T>,
{
  fn default() -> Self {
    Self::new(T::default())
  }
}

impl<B, T> From<T> for Synchronized<B, T>
where
  T: Sized,
  B: SynchronizedMutexBackend<T>,
{
  fn from(value: T) -> Self {
    Self::new(value)
  }
}

#[derive(Debug)]
pub struct SynchronizedRw<B, T: ?Sized>
where
  B: SynchronizedRwBackend<T>, {
  backend: B,
  _marker: PhantomData<T>,
}

impl<B, T> SynchronizedRw<B, T>
where
  T: ?Sized,
  B: SynchronizedRwBackend<T>,
{
  pub fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      backend: B::new(value),
      _marker: PhantomData,
    }
  }

  pub fn from_backend(backend: B) -> Self {
    Self {
      backend,
      _marker: PhantomData,
    }
  }

  pub fn backend(&self) -> &B {
    &self.backend
  }

  pub async fn read<R>(&self, f: impl FnOnce(&B::ReadGuard<'_>) -> R) -> R {
    let guard = self.backend.read().await;
    f(&guard)
  }

  pub async fn write<R>(&self, f: impl FnOnce(&mut B::WriteGuard<'_>) -> R) -> R {
    let mut guard = self.backend.write().await;
    f(&mut guard)
  }

  pub async fn read_guard(&self) -> GuardHandle<B::ReadGuard<'_>> {
    let guard = self.backend.read().await;
    GuardHandle::new(guard)
  }

  pub async fn write_guard(&self) -> GuardHandle<B::WriteGuard<'_>> {
    let guard = self.backend.write().await;
    GuardHandle::new(guard)
  }
}

impl<B, T> Default for SynchronizedRw<B, T>
where
  T: Default,
  B: SynchronizedRwBackend<T>,
{
  fn default() -> Self {
    Self::new(T::default())
  }
}

impl<B, T> From<T> for SynchronizedRw<B, T>
where
  T: Sized,
  B: SynchronizedRwBackend<T>,
{
  fn from(value: T) -> Self {
    Self::new(value)
  }
}
