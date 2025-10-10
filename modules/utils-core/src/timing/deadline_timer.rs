#[cfg(not(target_has_atomic = "ptr"))]
use core::cell::Cell;
#[cfg(target_has_atomic = "ptr")]
use core::sync::atomic::{AtomicUsize, Ordering};
use core::{
  fmt,
  task::{Context, Poll},
  time::Duration,
};
#[cfg(not(target_has_atomic = "ptr"))]
use critical_section::with;

/// Key for identifying items registered in a DeadlineTimer.
///
/// This key is used to uniquely identify each item registered in the timer.
/// Internally represented as a 64-bit integer, with 0 reserved as an invalid key.
///
/// # Examples
///
/// ```
/// use nexus_utils_core_rs::DeadlineTimerKey;
///
/// let key = DeadlineTimerKey::from_raw(42);
/// assert!(key.is_valid());
/// assert_eq!(key.into_raw(), 42);
///
/// let invalid = DeadlineTimerKey::invalid();
/// assert!(!invalid.is_valid());
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct DeadlineTimerKey(u64);

impl DeadlineTimerKey {
  /// Returns an invalid key.
  ///
  /// An invalid key is internally represented as 0 and is used to indicate
  /// a state where the item is not registered in the timer.
  #[inline]
  pub const fn invalid() -> Self {
    Self(0)
  }

  /// Checks if the key is valid.
  ///
  /// # Returns
  ///
  /// Returns `true` if the key is valid (non-zero), `false` if invalid (zero).
  #[inline]
  pub const fn is_valid(self) -> bool {
    self.0 != 0
  }

  /// Accesses the internal representation.
  ///
  /// Retrieves the internal 64-bit integer representation of the key.
  ///
  /// # Returns
  ///
  /// Returns the internal `u64` representation of the key.
  #[inline]
  pub const fn into_raw(self) -> u64 {
    self.0
  }

  /// Creates a key from an internal representation.
  ///
  /// # Arguments
  ///
  /// * `raw` - The 64-bit integer to use as the internal representation
  ///
  /// # Returns
  ///
  /// Returns a key created from the specified integer value.
  #[inline]
  pub const fn from_raw(raw: u64) -> Self {
    Self(raw)
  }
}

/// A newtype representing a DeadlineTimer deadline.
///
/// A wrapper type for type-safe handling of timer deadlines.
/// Internally represented as a `Duration`, representing the time
/// until a registered item expires.
///
/// # Examples
///
/// ```
/// use nexus_utils_core_rs::TimerDeadline;
/// use core::time::Duration;
///
/// let duration = Duration::from_secs(5);
/// let deadline = TimerDeadline::from_duration(duration);
/// assert_eq!(deadline.as_duration(), duration);
///
/// // Conversion via From/Into traits is also possible
/// let deadline2: TimerDeadline = Duration::from_millis(100).into();
/// let duration2: Duration = deadline2.into();
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerDeadline(Duration);

impl TimerDeadline {
  /// Creates a deadline from the specified duration.
  ///
  /// # Arguments
  ///
  /// * `duration` - The duration until the deadline
  ///
  /// # Returns
  ///
  /// Returns a `TimerDeadline` wrapping the specified duration.
  #[inline]
  pub const fn from_duration(duration: Duration) -> Self {
    Self(duration)
  }

  /// Retrieves the stored duration.
  ///
  /// # Returns
  ///
  /// Returns the wrapped `Duration` value.
  #[inline]
  pub const fn as_duration(self) -> Duration {
    self.0
  }
}

/// Provides conversion from `Duration` to `TimerDeadline`.
impl From<Duration> for TimerDeadline {
  #[inline]
  fn from(value: Duration) -> Self {
    Self::from_duration(value)
  }
}

/// Provides conversion from `TimerDeadline` to `Duration`.
impl From<TimerDeadline> for Duration {
  #[inline]
  fn from(value: TimerDeadline) -> Self {
    value.as_duration()
  }
}

/// Errors that may occur during DeadlineTimer operations.
///
/// Represents errors that can occur during timer operations.
///
/// # Variants
///
/// * `KeyNotFound` - When no item exists for the specified key
/// * `Closed` - When the timer cannot be operated on (e.g., already stopped)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeadlineTimerError {
  /// The entry corresponding to the specified key does not exist.
  ///
  /// Returned during `reset` or `cancel` operations when the specified key
  /// is not registered in the timer.
  KeyNotFound,
  /// The DeadlineTimer cannot be operated on (e.g., already stopped).
  ///
  /// Returned when attempting an operation after the timer has been closed.
  Closed,
}

impl fmt::Display for DeadlineTimerError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      DeadlineTimerError::KeyNotFound => write!(f, "key not found"),
      DeadlineTimerError::Closed => write!(f, "deadline timer is closed"),
    }
  }
}

#[cfg(feature = "std")]
impl std::error::Error for DeadlineTimerError {}

/// DeadlineTimer expiration event.
///
/// An event returned when an item registered in the timer expires.
/// Contains the expired item and the key to identify that item.
///
/// # Fields
///
/// * `key` - The key of the expired item
/// * `item` - The expired item itself
#[derive(Debug)]
pub struct DeadlineTimerExpired<Item> {
  /// The key of the expired item
  pub key: DeadlineTimerKey,
  /// The expired item itself
  pub item: Item,
}

/// Trait abstracting DeadlineTimer behavior.
///
/// Defines only the operations for insertion, reset, cancellation, and deadline waiting
/// to allow mechanisms like `ReceiveTimeout` that manage elements with deadlines
/// to swap out different runtime-specific implementations.
/// The API is limited to no standard library dependencies for `no_std` compatibility.
///
/// # Associated Types
///
/// * `Item` - The type of items managed by the timer
/// * `Error` - The error type that may occur during timer operations
///
/// # Examples
///
/// ```ignore
/// use nexus_utils_core_rs::{DeadlineTimer, TimerDeadline};
/// use core::time::Duration;
///
/// fn schedule_timeout<T: DeadlineTimer>(timer: &mut T, item: T::Item) {
///     let deadline = TimerDeadline::from_duration(Duration::from_secs(5));
///     let key = timer.insert(item, deadline).expect("Failed to insert");
///     // Save the key if needed for later cancellation or reset
/// }
/// ```
pub trait DeadlineTimer {
  /// The type of elements held by the timer.
  type Item;
  /// The error type that may occur during operations.
  type Error;

  /// Inserts a new element with a deadline.
  ///
  /// # Arguments
  ///
  /// * `item` - The item to register in the timer
  /// * `deadline` - The deadline until the item expires
  ///
  /// # Returns
  ///
  /// On success, returns a `DeadlineTimerKey` to identify the registered item.
  /// On failure, returns an error.
  fn insert(&mut self, item: Self::Item, deadline: TimerDeadline) -> Result<DeadlineTimerKey, Self::Error>;

  /// Updates and re-registers the deadline for an element with the specified key.
  ///
  /// # Arguments
  ///
  /// * `key` - The key of the item to update
  /// * `deadline` - The new deadline
  ///
  /// # Returns
  ///
  /// On success, returns `Ok(())`. On failure, returns an error.
  /// If the key doesn't exist, returns a `KeyNotFound` error.
  fn reset(&mut self, key: DeadlineTimerKey, deadline: TimerDeadline) -> Result<(), Self::Error>;

  /// Cancels an element with the specified key and returns it.
  ///
  /// # Arguments
  ///
  /// * `key` - The key of the item to cancel
  ///
  /// # Returns
  ///
  /// On success, returns the cancelled item in `Some`.
  /// If the key doesn't exist, returns `None`.
  /// If the timer is closed, returns an error.
  fn cancel(&mut self, key: DeadlineTimerKey) -> Result<Option<Self::Item>, Self::Error>;

  /// Polls for the element with the closest deadline.
  ///
  /// # Arguments
  ///
  /// * `cx` - The async task context
  ///
  /// # Returns
  ///
  /// * `Poll::Ready(Ok(expired))` - When an expired item is available
  /// * `Poll::Pending` - When no expired items yet
  /// * `Poll::Ready(Err(e))` - When an error occurs
  fn poll_expired(&mut self, cx: &mut Context<'_>) -> Poll<Result<DeadlineTimerExpired<Self::Item>, Self::Error>>;
}

/// Allocator for generating DeadlineTimerKeys.
///
/// An allocator for thread-safe generation of unique keys.
/// Internally uses an atomic counter to issue non-duplicate keys.
///
/// # Examples
///
/// ```
/// use nexus_utils_core_rs::DeadlineTimerKeyAllocator;
///
/// let allocator = DeadlineTimerKeyAllocator::new();
/// let key1 = allocator.allocate();
/// let key2 = allocator.allocate();
/// assert_ne!(key1, key2);
/// assert!(key1.is_valid());
/// assert!(key2.is_valid());
/// ```
#[derive(Debug)]
pub struct DeadlineTimerKeyAllocator {
  #[cfg(target_has_atomic = "ptr")]
  counter: AtomicUsize,
  #[cfg(not(target_has_atomic = "ptr"))]
  counter: Cell<usize>,
}

impl DeadlineTimerKeyAllocator {
  /// Creates a new allocator.
  ///
  /// The counter starts at 1, with 0 reserved as an invalid key.
  ///
  /// # Returns
  ///
  /// Returns a newly created `DeadlineTimerKeyAllocator`.
  #[inline]
  pub const fn new() -> Self {
    #[cfg(target_has_atomic = "ptr")]
    {
      Self {
        counter: AtomicUsize::new(1),
      }
    }

    #[cfg(not(target_has_atomic = "ptr"))]
    {
      Self { counter: Cell::new(1) }
    }
  }

  /// Issues a new unique key.
  ///
  /// This operation is thread-safe, and even when called from multiple threads simultaneously,
  /// always returns a unique key.
  ///
  /// # Returns
  ///
  /// Returns a newly generated unique `DeadlineTimerKey`.
  ///
  /// # Panics
  ///
  /// Even if the counter overflows, it safely restarts from 1.
  #[inline]
  pub fn allocate(&self) -> DeadlineTimerKey {
    #[cfg(target_has_atomic = "ptr")]
    {
      let next = self.counter.fetch_add(1, Ordering::Relaxed) as u64;
      let raw = if next == 0 { 1 } else { next };
      DeadlineTimerKey::from_raw(raw)
    }

    #[cfg(not(target_has_atomic = "ptr"))]
    {
      let issued = with(|_| {
        let current = self.counter.get();
        let next = current.wrapping_add(1);
        let stored = if next == 0 { 1 } else { next };
        self.counter.set(stored);
        if current == 0 {
          1
        } else {
          current
        }
      });
      DeadlineTimerKey::from_raw(issued as u64)
    }
  }

  /// Checks the next key to be issued (for testing purposes).
  ///
  /// This operation doesn't actually issue a key, just checks what key
  /// `allocate` would return next. Mainly used for testing and debugging.
  ///
  /// # Returns
  ///
  /// Returns the `DeadlineTimerKey` expected to be returned by the next `allocate`.
  ///
  /// # Note
  ///
  /// Since other threads may intervene between this operation and an actual `allocate`,
  /// the returned key may not actually be the next one issued.
  #[inline]
  pub fn peek(&self) -> DeadlineTimerKey {
    #[cfg(target_has_atomic = "ptr")]
    {
      DeadlineTimerKey::from_raw(self.counter.load(Ordering::Relaxed) as u64)
    }

    #[cfg(not(target_has_atomic = "ptr"))]
    {
      with(|_| DeadlineTimerKey::from_raw(self.counter.get() as u64))
    }
  }
}

/// Implementation of the `Default` trait.
///
/// Creates a new allocator with the same behavior as `new()`.
impl Default for DeadlineTimerKeyAllocator {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  extern crate std;

  use super::*;
  use std::collections::HashSet;

  #[test]
  fn allocate_provides_unique_keys() {
    let allocator = DeadlineTimerKeyAllocator::new();
    let mut keys = HashSet::new();

    for _ in 0..1024 {
      let key = allocator.allocate();
      assert!(key.is_valid());
      assert!(keys.insert(key.into_raw()));
    }
  }

  #[test]
  fn deadline_roundtrip() {
    let duration = Duration::from_millis(150);
    let deadline = TimerDeadline::from(duration);
    assert_eq!(deadline.as_duration(), duration);
    let back: Duration = deadline.into();
    assert_eq!(back, duration);
  }
}
