use alloc::boxed::Box;
use async_trait::async_trait;

/// Trait defining WaitGroup backend implementation
///
/// This trait is used to provide concrete WaitGroup implementations
/// for different async runtimes (Tokio, async-std, embedded systems, etc.).
///
/// # Design Philosophy
///
/// - Provides concurrent control mechanism similar to Go's WaitGroup
/// - Portability through runtime-independent abstraction
/// - Easy sharing across multiple tasks through clonable design
#[async_trait(?Send)]
pub trait WaitGroupBackend: Clone {
  /// Creates a new backend instance
  ///
  /// The counter is initialized to 0.
  fn new() -> Self;

  /// Creates a new backend instance with the specified count
  ///
  /// # Arguments
  ///
  /// * `count` - Initial counter value
  fn with_count(count: usize) -> Self;

  /// Adds the specified number to the counter
  ///
  /// # Arguments
  ///
  /// * `n` - The value to add
  fn add(&self, n: usize);

  /// Decrements the counter by 1
  ///
  /// When the counter reaches 0, all tasks waiting on `wait()` are resumed.
  fn done(&self);

  /// Waits until the counter reaches 0
  ///
  /// If the counter is already 0, this function returns immediately.
  async fn wait(&self);
}

/// Synchronization primitive for waiting on multiple concurrent tasks
///
/// `WaitGroup` is a synchronization mechanism inspired by Go's `sync.WaitGroup`,
/// used to wait for the completion of multiple async tasks.
///
/// # Usage Pattern
///
/// 1. Add the number of tasks to wait for with `add(n)`
/// 2. Each task calls `done()` upon completion
/// 3. Wait for all tasks to complete with `wait()`
///
/// # Examples
///
/// ```rust,ignore
/// let wg = WaitGroup::<TokioWaitGroupBackend>::new();
/// wg.add(3);
///
/// for i in 0..3 {
///     let wg_clone = wg.clone();
///     tokio::spawn(async move {
///         // Some processing
///         wg_clone.done();
///     });
/// }
///
/// wg.wait().await; // Wait for all tasks to complete
/// ```
///
/// # Type Parameters
///
/// * `B` - Concrete backend type implementing the WaitGroupBackend trait
#[derive(Clone, Debug)]
pub struct WaitGroup<B>
where
  B: WaitGroupBackend, {
  backend: B,
}

impl<B> WaitGroup<B>
where
  B: WaitGroupBackend,
{
  /// Creates a new WaitGroup
  ///
  /// The counter is initialized to 0.
  ///
  /// # Returns
  ///
  /// A new WaitGroup instance
  pub fn new() -> Self {
    Self { backend: B::new() }
  }

  /// Creates a new WaitGroup with the specified count
  ///
  /// # Arguments
  ///
  /// * `count` - Initial count value (number of tasks to wait for)
  ///
  /// # Returns
  ///
  /// A WaitGroup instance initialized with the specified count
  pub fn with_count(count: usize) -> Self {
    Self {
      backend: B::with_count(count),
    }
  }

  /// Adds the specified number to the counter
  ///
  /// Increments the counter by the number of tasks to be started.
  ///
  /// # Arguments
  ///
  /// * `n` - The value to add (number of tasks to start)
  pub fn add(&self, n: usize) {
    self.backend.add(n);
  }

  /// Decrements the counter by 1
  ///
  /// Called when a task completes. When the counter reaches 0,
  /// all tasks waiting on `wait()` are resumed.
  pub fn done(&self) {
    self.backend.done();
  }

  /// Asynchronously waits until the counter reaches 0
  ///
  /// If the counter is already 0, this function returns immediately.
  /// Otherwise, it waits until all tasks have called `done()`.
  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  /// Gets a reference to the backend
  ///
  /// # Returns
  ///
  /// An immutable reference to the backend instance
  pub fn backend(&self) -> &B {
    &self.backend
  }
}

impl<B> Default for WaitGroup<B>
where
  B: WaitGroupBackend,
{
  fn default() -> Self {
    Self::new()
  }
}
