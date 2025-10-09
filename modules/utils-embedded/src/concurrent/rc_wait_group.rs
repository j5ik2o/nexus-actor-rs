use alloc::boxed::Box;
use alloc::rc::Rc;

use core::cell::RefCell;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::signal::Signal;
use nexus_utils_core_rs::{async_trait, WaitGroup as CoreWaitGroup, WaitGroupBackend};

/// Backend for `Rc`-based wait group implementation.
///
/// Provides a mechanism in `no_std` environments to track and wait for the completion of multiple tasks.
/// The count can be dynamically increased or decreased, and waiting tasks are released when the count reaches 0.
///
/// # Features
///
/// - Reference counting via `Rc` (single-threaded only)
/// - Asynchronous signaling via Embassy's `Signal`
/// - Dynamic count management (`add` to increment, `done` to decrement)
///
/// # Usage Examples
///
/// ```ignore
/// let wg = WaitGroup::new();
/// wg.add(2);
///
/// let clone = wg.clone();
/// async move {
///   // Task 1
///   clone.done();
///   // Task 2
///   clone.done();
/// };
///
/// // Wait for all tasks to complete
/// wg.wait().await;
/// ```
#[derive(Clone)]
pub struct RcWaitGroupBackend {
  count: Rc<RefCell<usize>>,
  signal: Rc<Signal<NoopRawMutex, ()>>,
}

#[async_trait(?Send)]
impl WaitGroupBackend for RcWaitGroupBackend {
  /// Creates a new wait group backend with count 0.
  fn new() -> Self {
    Self::with_count(0)
  }

  /// Creates a new wait group backend with the specified count.
  ///
  /// # Arguments
  ///
  /// * `count` - Initial count value
  fn with_count(count: usize) -> Self {
    Self {
      count: Rc::new(RefCell::new(count)),
      signal: Rc::new(Signal::new()),
    }
  }

  /// Increments the count by the specified amount.
  ///
  /// # Arguments
  ///
  /// * `n` - Number of counts to add
  fn add(&self, n: usize) {
    *self.count.borrow_mut() += n;
  }

  /// Decrements the count by 1.
  ///
  /// When the count reaches 0, all waiting tasks receive a signal and are released.
  ///
  /// # Panics
  ///
  /// Panics if called when the count is already 0.
  fn done(&self) {
    let mut count = self.count.borrow_mut();
    assert!(*count > 0, "WaitGroup::done called more times than add");
    *count -= 1;
    if *count == 0 {
      self.signal.signal(());
    }
  }

  /// Waits until the count reaches 0.
  ///
  /// Returns immediately if the count is already 0.
  async fn wait(&self) {
    let count = self.count.clone();
    let signal = self.signal.clone();
    loop {
      if *count.borrow() == 0 {
        return;
      }
      signal.wait().await;
    }
  }
}

/// Type alias for `Rc`-based wait group.
///
/// Wait group implementation usable in `no_std` environments.
/// Provides functionality to track completion of multiple tasks and wait until all are done.
pub type WaitGroup = CoreWaitGroup<RcWaitGroupBackend>;

#[cfg(test)]
mod tests {
  use super::WaitGroup;
  use futures::executor::block_on;
  use futures::join;

  #[test]
  fn wait_group_completes() {
    block_on(async {
      let wg = WaitGroup::new();
      wg.add(2);
      let clone = wg.clone();
      let worker = async move {
        clone.done();
        clone.done();
      };
      join!(worker, wg.wait());
    });
  }
}
