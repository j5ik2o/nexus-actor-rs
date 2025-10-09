use alloc::boxed::Box;
use async_trait::async_trait;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

/// Handle that wraps a guard object
///
/// This handle holds a mutex or RwLock guard and allows access to
/// the inner value through `Deref` and `DerefMut`.
#[derive(Debug)]
pub struct GuardHandle<G> {
  guard: G,
}

impl<G> GuardHandle<G> {
  /// Creates a new `GuardHandle`.
  ///
  /// # Arguments
  ///
  /// * `guard` - Guard object to wrap
  pub fn new(guard: G) -> Self {
    Self { guard }
  }

  /// Extracts the guard object.
  ///
  /// # Returns
  ///
  /// The inner guard object
  pub fn into_inner(self) -> G {
    self.guard
  }
}

impl<G> Deref for GuardHandle<G> {
  type Target = G;

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

/// Backend trait for async mutex-like primitives
///
/// This trait defines the backend implementation for async mutex used by
/// the `Synchronized` type. Concrete implementations are provided by Tokio,
/// async-std, or custom runtimes.
#[async_trait(?Send)]
pub trait SynchronizedMutexBackend<T: ?Sized> {
  /// Guard type returned when lock is acquired
  type Guard<'a>: Deref<Target = T> + DerefMut + 'a
  where
    Self: 'a;

  /// Creates a new backend with the specified value.
  ///
  /// # Arguments
  ///
  /// * `value` - Initial value to be protected by the mutex
  fn new(value: T) -> Self
  where
    T: Sized;

  /// Locks the mutex and obtains a guard.
  ///
  /// # Returns
  ///
  /// A guard providing access to the protected value
  async fn lock(&self) -> Self::Guard<'_>;
}

/// Backend trait for async read/write lock primitives
///
/// This trait defines the backend implementation for async RwLock used by
/// the `SynchronizedRw` type. By separating read and write access, it enables
/// multiple readers and a single writer.
#[async_trait(?Send)]
pub trait SynchronizedRwBackend<T: ?Sized> {
  /// Guard type returned when read lock is acquired
  type ReadGuard<'a>: Deref<Target = T> + 'a
  where
    Self: 'a;

  /// Guard type returned when write lock is acquired
  type WriteGuard<'a>: Deref<Target = T> + DerefMut + 'a
  where
    Self: 'a;

  /// Creates a new backend with the specified value.
  ///
  /// # Arguments
  ///
  /// * `value` - Initial value to be protected by the RwLock
  fn new(value: T) -> Self
  where
    T: Sized;

  /// Acquires a read lock.
  ///
  /// # Returns
  ///
  /// A guard providing read-only access to the protected value
  async fn read(&self) -> Self::ReadGuard<'_>;

  /// Acquires a write lock.
  ///
  /// # Returns
  ///
  /// A guard providing exclusive access to the protected value
  async fn write(&self) -> Self::WriteGuard<'_>;
}

/// Async synchronization primitive providing backend abstraction
///
/// `Synchronized` provides exclusive access to a value using a mutex-like backend.
/// Backends can be adapted to different runtimes or custom implementations by
/// implementing the `SynchronizedMutexBackend` trait.
///
/// # Type Parameters
///
/// * `B` - Backend implementation to use
/// * `T` - Type of value to be protected
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
  /// Creates a new `Synchronized` with the specified value.
  ///
  /// # Arguments
  ///
  /// * `value` - Initial value to protect
  pub fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      backend: B::new(value),
      _marker: PhantomData,
    }
  }

  /// Creates a `Synchronized` from an existing backend.
  ///
  /// # Arguments
  ///
  /// * `backend` - Backend instance to use
  pub fn from_backend(backend: B) -> Self {
    Self {
      backend,
      _marker: PhantomData,
    }
  }

  /// Gets a reference to the internal backend.
  ///
  /// # Returns
  ///
  /// Immutable reference to the backend
  pub fn backend(&self) -> &B {
    &self.backend
  }

  /// Acquires a lock and executes the specified function (for reading).
  ///
  /// # Arguments
  ///
  /// * `f` - Function that receives a reference to the guard
  ///
  /// # Returns
  ///
  /// Return value of function `f`
  pub async fn read<R>(&self, f: impl FnOnce(&B::Guard<'_>) -> R) -> R {
    let guard = self.backend.lock().await;
    f(&guard)
  }

  /// Acquires a lock and executes the specified function (for writing).
  ///
  /// # Arguments
  ///
  /// * `f` - Function that receives a mutable reference to the guard
  ///
  /// # Returns
  ///
  /// Return value of function `f`
  pub async fn write<R>(&self, f: impl FnOnce(&mut B::Guard<'_>) -> R) -> R {
    let mut guard = self.backend.lock().await;
    f(&mut guard)
  }

  /// Acquires a lock and returns a guard handle.
  ///
  /// Use this method when you need to hold the guard for an extended period.
  ///
  /// # Returns
  ///
  /// `GuardHandle` wrapping the guard object
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

/// Async read/write synchronization primitive providing backend abstraction
///
/// `SynchronizedRw` provides separate read and write access to a value using
/// an RwLock-like backend. Multiple reads are allowed concurrently,
/// while writes are exclusive.
///
/// # Type Parameters
///
/// * `B` - Backend implementation to use
/// * `T` - Type of value to be protected
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
  /// Creates a new `SynchronizedRw` with the specified value.
  ///
  /// # Arguments
  ///
  /// * `value` - Initial value to protect
  pub fn new(value: T) -> Self
  where
    T: Sized, {
    Self {
      backend: B::new(value),
      _marker: PhantomData,
    }
  }

  /// Creates a `SynchronizedRw` from an existing backend.
  ///
  /// # Arguments
  ///
  /// * `backend` - Backend instance to use
  pub fn from_backend(backend: B) -> Self {
    Self {
      backend,
      _marker: PhantomData,
    }
  }

  /// Gets a reference to the internal backend.
  ///
  /// # Returns
  ///
  /// Immutable reference to the backend
  pub fn backend(&self) -> &B {
    &self.backend
  }

  /// Acquires a read lock and executes the specified function.
  ///
  /// # Arguments
  ///
  /// * `f` - Function that receives a reference to the read guard
  ///
  /// # Returns
  ///
  /// Return value of function `f`
  pub async fn read<R>(&self, f: impl FnOnce(&B::ReadGuard<'_>) -> R) -> R {
    let guard = self.backend.read().await;
    f(&guard)
  }

  /// Acquires a write lock and executes the specified function.
  ///
  /// # Arguments
  ///
  /// * `f` - Function that receives a mutable reference to the write guard
  ///
  /// # Returns
  ///
  /// Return value of function `f`
  pub async fn write<R>(&self, f: impl FnOnce(&mut B::WriteGuard<'_>) -> R) -> R {
    let mut guard = self.backend.write().await;
    f(&mut guard)
  }

  /// Acquires a read lock and returns a guard handle.
  ///
  /// Use this method when you need to hold the read guard for an extended period.
  ///
  /// # Returns
  ///
  /// `GuardHandle` wrapping the read guard object
  pub async fn read_guard(&self) -> GuardHandle<B::ReadGuard<'_>> {
    let guard = self.backend.read().await;
    GuardHandle::new(guard)
  }

  /// Acquires a write lock and returns a guard handle.
  ///
  /// Use this method when you need to hold the write guard for an extended period.
  ///
  /// # Returns
  ///
  /// `GuardHandle` wrapping the write guard object
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
