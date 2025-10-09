use alloc::boxed::Box;
use async_trait::async_trait;

/// Trait defining the backend implementation for CountDownLatch
///
/// This trait abstracts the concrete implementation of CountDownLatch to support
/// different environments (standard library, embedded environments, etc.).
///
/// # Examples
///
/// ```ignore
/// use async_trait::async_trait;
///
/// #[derive(Clone)]
/// struct MyBackend {
///     // Backend implementation details
/// }
///
/// #[async_trait(?Send)]
/// impl CountDownLatchBackend for MyBackend {
///     fn new(count: usize) -> Self {
///         // Initialization logic
///     }
///
///     async fn count_down(&self) {
///         // Count down logic
///     }
///
///     async fn wait(&self) {
///         // Wait logic
///     }
/// }
/// ```
#[async_trait(?Send)]
pub trait CountDownLatchBackend: Clone {
  /// Initializes the backend with the specified count value
  ///
  /// # Arguments
  ///
  /// * `count` - Initial count value. Counts down until it reaches 0
  fn new(count: usize) -> Self;

  /// Decrements the count by 1
  ///
  /// When the count reaches 0, all waiting tasks are released.
  async fn count_down(&self);

  /// Waits until the count reaches 0
  ///
  /// This method blocks the current task until the count becomes 0.
  /// Multiple tasks can wait simultaneously.
  async fn wait(&self);
}

/// Count-down latch synchronization primitive
///
/// `CountDownLatch` is a synchronization mechanism that causes multiple tasks to wait
/// until a specified count reaches zero. It provides functionality equivalent to Java's
/// `CountDownLatch` or Go's `WaitGroup`.
///
/// # Usage Examples
///
/// ```ignore
/// use nexus_utils_core_rs::CountDownLatch;
///
/// #[tokio::main]
/// async fn main() {
///     let latch = CountDownLatch::<SomeBackend>::new(3);
///
///     // Spawn 3 tasks
///     for i in 0..3 {
///         let latch_clone = latch.clone();
///         tokio::spawn(async move {
///             // Some processing
///             println!("Task {} completed", i);
///             latch_clone.count_down().await;
///         });
///     }
///
///     // Wait for all tasks to complete
///     latch.wait().await;
///     println!("All tasks completed");
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CountDownLatch<B>
where
  B: CountDownLatchBackend, {
  backend: B,
}

impl<B> CountDownLatch<B>
where
  B: CountDownLatchBackend,
{
  /// Creates a new `CountDownLatch` with the specified count value
  ///
  /// # Arguments
  ///
  /// * `count` - Initial count value. `count_down` must be called this many times to reach 0
  ///
  /// # Examples
  ///
  /// ```ignore
  /// let latch = CountDownLatch::<SomeBackend>::new(5);
  /// ```
  pub fn new(count: usize) -> Self {
    Self { backend: B::new(count) }
  }

  /// Decrements the count by 1
  ///
  /// When the count reaches 0, all tasks waiting in `wait()` are released.
  /// If the count is already 0, this method does nothing.
  ///
  /// # Examples
  ///
  /// ```ignore
  /// latch.count_down().await;
  /// ```
  pub async fn count_down(&self) {
    self.backend.count_down().await;
  }

  /// Causes the current task to wait until the count reaches 0
  ///
  /// If the count is already 0, this method returns immediately.
  /// Multiple tasks can wait simultaneously, and all are released at once when the count reaches 0.
  ///
  /// # Examples
  ///
  /// ```ignore
  /// latch.wait().await;
  /// println!("All operations completed");
  /// ```
  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  /// Gets a reference to the internal backend
  ///
  /// # Returns
  ///
  /// Immutable reference to the backend implementation
  pub fn backend(&self) -> &B {
    &self.backend
  }
}

impl<B> Default for CountDownLatch<B>
where
  B: CountDownLatchBackend,
{
  /// Creates a default `CountDownLatch` with a count of 0
  ///
  /// This default implementation creates a latch with count 0.
  /// This means `wait()` will return immediately.
  fn default() -> Self {
    Self::new(0)
  }
}
