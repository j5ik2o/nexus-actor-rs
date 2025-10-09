use alloc::sync::Arc;

use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex, RawMutex};
use embassy_sync::mutex::{Mutex, MutexGuard};
use nexus_utils_core_rs::{
  MpscBackend, MpscBuffer, MpscHandle, QueueHandle, QueueStorage, RingBackend, RingBuffer, RingBufferStorage,
  RingHandle, StackBackend, StackHandle,
};
use nexus_utils_core_rs::{Shared, StateCell};

/// `Arc`-based shared reference type for embedded environments.
///
/// This type wraps the standard library's `Arc` to provide thread-safe reference counting
/// for shared ownership of values in embedded systems. It implements the [`Shared`] trait
/// and various handle traits for integration with queues, stacks, and other data structures.
///
/// # Examples
///
/// ```
/// use nexus_utils_embedded_rs::sync::ArcShared;
///
/// let shared = ArcShared::new(42);
/// let clone = shared.clone();
/// assert_eq!(*shared, 42);
/// assert_eq!(*clone, 42);
/// ```
pub struct ArcShared<T: ?Sized>(Arc<T>);

impl<T: ?Sized> core::fmt::Debug for ArcShared<T> {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("ArcShared").finish()
  }
}

impl<T> ArcShared<T>
where
  T: Sized,
{
  /// Creates a new `ArcShared` containing the given value.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::sync::ArcShared;
  ///
  /// let shared = ArcShared::new(42);
  /// assert_eq!(*shared, 42);
  /// ```
  pub fn new(value: T) -> Self {
    Self(Arc::new(value))
  }
}

impl<T: ?Sized> ArcShared<T> {
  /// Creates a new `ArcShared` from an existing `Arc`.
  ///
  /// This allows wrapping an already-allocated `Arc` without additional allocation.
  ///
  /// # Examples
  ///
  /// ```
  /// use alloc::sync::Arc;
  /// use nexus_utils_embedded_rs::sync::ArcShared;
  ///
  /// let arc = Arc::new(42);
  /// let shared = ArcShared::from_arc(arc);
  /// assert_eq!(*shared, 42);
  /// ```
  pub fn from_arc(inner: Arc<T>) -> Self {
    Self(inner)
  }

  /// Consumes this `ArcShared` and returns the underlying `Arc`.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::sync::ArcShared;
  ///
  /// let shared = ArcShared::new(42);
  /// let arc = shared.into_arc();
  /// assert_eq!(*arc, 42);
  /// ```
  pub fn into_arc(self) -> Arc<T> {
    self.0
  }
}

impl<T: ?Sized> Clone for ArcShared<T> {
  fn clone(&self) -> Self {
    Self(self.0.clone())
  }
}

impl<T: ?Sized> core::ops::Deref for ArcShared<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T: ?Sized> Shared<T> for ArcShared<T> {
  fn try_unwrap(self) -> Result<T, Self>
  where
    T: Sized, {
    Arc::try_unwrap(self.0).map_err(ArcShared)
  }
}

impl<T, E> QueueHandle<E> for ArcShared<T>
where
  T: QueueStorage<E>,
{
  type Storage = T;

  fn storage(&self) -> &Self::Storage {
    &self.0
  }
}

impl<T, B> MpscHandle<T> for ArcShared<B>
where
  B: MpscBackend<T>,
{
  type Backend = B;

  fn backend(&self) -> &Self::Backend {
    &self.0
  }
}

impl<E, B> RingHandle<E> for ArcShared<B>
where
  B: RingBackend<E> + ?Sized,
{
  type Backend = B;

  fn backend(&self) -> &Self::Backend {
    &self.0
  }
}

impl<T, B> StackHandle<T> for ArcShared<B>
where
  B: StackBackend<T> + ?Sized,
{
  type Backend = B;

  fn backend(&self) -> &Self::Backend {
    &self.0
  }
}

/// `Arc`-based mutable state cell using embassy-sync's `Mutex`.
///
/// This type combines `Arc` with [`embassy_sync::mutex::Mutex`] to provide shared mutable
/// state with thread-safe reference counting. The mutex type is parameterized by a
/// [`RawMutex`] implementation, allowing different synchronization strategies:
///
/// - [`NoopRawMutex`]: No synchronization (single-threaded or cooperative multitasking)
/// - [`CriticalSectionRawMutex`]: Uses critical sections for interrupt safety
///
/// # Type Parameters
///
/// - `T`: The type of value stored in the cell
/// - `RM`: The raw mutex implementation (defaults to [`NoopRawMutex`])
///
/// # Examples
///
/// ```
/// use nexus_utils_embedded_rs::sync::ArcLocalStateCell;
///
/// let cell = ArcLocalStateCell::new(0);
/// let clone = cell.clone();
///
/// *clone.borrow_mut() = 42;
/// assert_eq!(*cell.borrow(), 42);
/// ```
#[derive(Debug)]
pub struct ArcStateCell<T, RM = NoopRawMutex>
where
  RM: RawMutex, {
  inner: Arc<Mutex<RM, T>>,
}

/// Type alias for `ArcStateCell` with [`NoopRawMutex`].
///
/// This variant provides no synchronization and is suitable for single-threaded
/// environments or cooperative multitasking where the executor guarantees non-preemption.
pub type ArcLocalStateCell<T> = ArcStateCell<T, NoopRawMutex>;

/// Type alias for `ArcStateCell` with [`CriticalSectionRawMutex`].
///
/// This variant uses critical sections for synchronization, making it safe to use
/// across interrupts and in preemptive multitasking environments.
pub type ArcCsStateCell<T> = ArcStateCell<T, CriticalSectionRawMutex>;

impl<T, RM> ArcStateCell<T, RM>
where
  RM: RawMutex,
{
  /// Creates a new `ArcStateCell` containing the given value.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::sync::ArcLocalStateCell;
  ///
  /// let cell = ArcLocalStateCell::new(42);
  /// assert_eq!(*cell.borrow(), 42);
  /// ```
  pub fn new(value: T) -> Self {
    Self {
      inner: Arc::new(Mutex::new(value)),
    }
  }

  /// Creates a new `ArcStateCell` from an existing `Arc<Mutex<RM, T>>`.
  ///
  /// This allows wrapping an already-allocated mutex without additional allocation.
  ///
  /// # Examples
  ///
  /// ```
  /// use alloc::sync::Arc;
  /// use embassy_sync::blocking_mutex::raw::NoopRawMutex;
  /// use embassy_sync::mutex::Mutex;
  /// use nexus_utils_embedded_rs::sync::ArcStateCell;
  ///
  /// let arc = Arc::new(Mutex::<NoopRawMutex, _>::new(42));
  /// let cell = ArcStateCell::from_arc(arc);
  /// assert_eq!(*cell.borrow(), 42);
  /// ```
  pub fn from_arc(inner: Arc<Mutex<RM, T>>) -> Self {
    Self { inner }
  }

  /// Consumes this `ArcStateCell` and returns the underlying `Arc<Mutex<RM, T>>`.
  ///
  /// # Examples
  ///
  /// ```
  /// use nexus_utils_embedded_rs::sync::ArcLocalStateCell;
  ///
  /// let cell = ArcLocalStateCell::new(42);
  /// let arc = cell.into_arc();
  /// assert_eq!(*arc.try_lock().unwrap(), 42);
  /// ```
  pub fn into_arc(self) -> Arc<Mutex<RM, T>> {
    self.inner
  }

  fn lock(&self) -> MutexGuard<'_, RM, T> {
    self
      .inner
      .try_lock()
      .unwrap_or_else(|_| panic!("ArcStateCell: concurrent access detected"))
  }
}

impl<T, RM> Clone for ArcStateCell<T, RM>
where
  RM: RawMutex,
{
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

impl<T, RM> StateCell<T> for ArcStateCell<T, RM>
where
  RM: RawMutex,
{
  type Ref<'a>
    = MutexGuard<'a, RM, T>
  where
    Self: 'a,
    T: 'a;
  type RefMut<'a>
    = MutexGuard<'a, RM, T>
  where
    Self: 'a,
    T: 'a;

  fn new(value: T) -> Self
  where
    Self: Sized, {
    ArcStateCell::new(value)
  }

  fn borrow(&self) -> Self::Ref<'_> {
    self.lock()
  }

  fn borrow_mut(&self) -> Self::RefMut<'_> {
    self.lock()
  }
}

impl<T, RM> RingBufferStorage<T> for ArcStateCell<MpscBuffer<T>, RM>
where
  RM: RawMutex,
{
  fn with_read<R>(&self, f: impl FnOnce(&MpscBuffer<T>) -> R) -> R {
    let guard = self.borrow();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut MpscBuffer<T>) -> R) -> R {
    let mut guard = self.borrow_mut();
    f(&mut guard)
  }
}

impl<E, RM> QueueStorage<E> for ArcStateCell<RingBuffer<E>, RM>
where
  RM: RawMutex,
{
  fn with_read<R>(&self, f: impl FnOnce(&RingBuffer<E>) -> R) -> R {
    let guard = self.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut RingBuffer<E>) -> R) -> R {
    let mut guard = self.lock();
    f(&mut guard)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::tests::init_arc_critical_section;

  fn prepare() {
    init_arc_critical_section();
  }

  #[test]
  fn arc_state_cell_updates() {
    prepare();
    let cell = ArcLocalStateCell::new(0_u32);
    let cloned = cell.clone();

    {
      let mut value = cloned.borrow_mut();
      *value = 9;
    }

    assert_eq!(*cell.borrow(), 9);
  }

  #[test]
  fn arc_shared_try_unwrap() {
    prepare();
    let shared = ArcShared::new(7_u32);
    assert_eq!(ArcShared::new(3_u32).try_unwrap().unwrap(), 3);
    let clone = shared.clone();
    assert!(clone.try_unwrap().is_err());
  }

  #[test]
  fn arc_state_cell_into_arc_exposes_inner() {
    prepare();
    let cell = ArcLocalStateCell::new(5_u32);
    let arc = cell.clone().into_arc();
    assert_eq!(*arc.try_lock().unwrap(), 5);
  }
}
