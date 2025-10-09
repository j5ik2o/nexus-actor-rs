use alloc::boxed::Box;
use async_trait::async_trait;

/// Trait defining the backend implementation for async barriers.
///
/// This trait is used to implement barrier synchronization backends
/// for different execution environments (Tokio, async-std, etc.).
///
/// # Implementation Requirements
///
/// - Must implement the `Clone` trait
/// - Need not be thread-sendable (`?Send`)
///
/// # Examples
///
/// ```ignore
/// use nexus_utils_core_rs::{AsyncBarrierBackend, AsyncBarrier};
///
/// // Example custom backend implementation
/// #[derive(Clone)]
/// struct MyBackend {
///     // Backend-specific fields
/// }
///
/// #[async_trait::async_trait(?Send)]
/// impl AsyncBarrierBackend for MyBackend {
///     fn new(count: usize) -> Self {
///         // Initialization logic
///         MyBackend { /* ... */ }
///     }
///
///     async fn wait(&self) {
///         // Wait logic implementation
///     }
/// }
/// ```
#[async_trait(?Send)]
pub trait AsyncBarrierBackend: Clone {
  /// Creates a backend that waits for the specified number of tasks.
  ///
  /// # Arguments
  ///
  /// * `count` - Number of tasks that must reach the barrier before it is released
  fn new(count: usize) -> Self;

  /// Waits at the barrier point.
  ///
  /// Tasks calling this method block until all tasks (specified count)
  /// have called `wait()`.
  /// When all tasks arrive, all tasks are released simultaneously.
  async fn wait(&self);
}

/// Structure providing synchronization barrier among async tasks.
///
/// `AsyncBarrier` provides a barrier synchronization mechanism for multiple async
/// tasks to synchronize at a specific point. All tasks wait until the specified
/// number of tasks reach the barrier, then all resume processing simultaneously.
///
/// # Type Parameters
///
/// * `B` - Backend implementation to use (a type implementing `AsyncBarrierBackend`)
///
/// # Examples
///
/// ```ignore
/// use nexus_utils_core_rs::AsyncBarrier;
/// use tokio::task;
///
/// #[tokio::main]
/// async fn main() {
///     // Create a barrier that waits for 3 tasks to arrive
///     let barrier = AsyncBarrier::new(3);
///
///     let mut handles = vec![];
///
///     for i in 0..3 {
///         let barrier_clone = barrier.clone();
///         let handle = task::spawn(async move {
///             println!("Task {} started", i);
///
///             // Wait at barrier
///             barrier_clone.wait().await;
///
///             println!("Task {} passed the barrier", i);
///         });
///         handles.push(handle);
///     }
///
///     // Wait for all tasks to complete
///     for handle in handles {
///         handle.await.unwrap();
///     }
/// }
/// ```
///
/// # Usage Example (Concurrent Processing Synchronization)
///
/// ```ignore
/// use nexus_utils_core_rs::AsyncBarrier;
///
/// async fn parallel_computation() {
///     let barrier = AsyncBarrier::new(3);
///
///     // Phase 1: Each task executes initialization
///     let tasks: Vec<_> = (0..3).map(|i| {
///         let barrier = barrier.clone();
///         tokio::spawn(async move {
///             // Initialization processing
///             println!("Task {} initializing...", i);
///
///             // Wait for all tasks to finish initialization
///             barrier.wait().await;
///
///             // Phase 2: Processing executed after all initialization completes
///             println!("Task {} executing main processing", i);
///         })
///     }).collect();
///
///     for task in tasks {
///         task.await.unwrap();
///     }
/// }
/// ```
#[derive(Clone, Debug)]
pub struct AsyncBarrier<B>
where
  B: AsyncBarrierBackend, {
  backend: B,
}

impl<B> AsyncBarrier<B>
where
  B: AsyncBarrierBackend,
{
  /// Creates a new barrier that waits for the specified number of tasks.
  ///
  /// # Arguments
  ///
  /// * `count` - Number of tasks that must reach the barrier before it is released
  ///
  /// # Returns
  ///
  /// A new `AsyncBarrier` instance
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_core_rs::AsyncBarrier;
  ///
  /// // Create a barrier that waits for 5 tasks to arrive
  /// let barrier = AsyncBarrier::new(5);
  /// ```
  ///
  /// # Panics
  ///
  /// If `count` is 0, some backend implementations may panic.
  pub fn new(count: usize) -> Self {
    Self { backend: B::new(count) }
  }

  /// Waits at the barrier point.
  ///
  /// Tasks calling this method enter a wait state until all tasks (the specified count)
  /// have called `wait()` on the barrier.
  /// When all tasks arrive, all tasks are released simultaneously and can
  /// continue processing.
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_core_rs::AsyncBarrier;
  ///
  /// let barrier = AsyncBarrier::new(3);
  ///
  /// // Call from multiple tasks
  /// let barrier_clone = barrier.clone();
  /// tokio::spawn(async move {
  ///     println!("Task 1: Waiting at barrier...");
  ///     barrier_clone.wait().await;
  ///     println!("Task 1: Passed the barrier!");
  /// });
  /// ```
  ///
  /// # Behavior
  ///
  /// - The first `count - 1` tasks enter a wait state when calling `wait()`
  /// - When the `count`-th task calls `wait()`, all tasks are released simultaneously
  /// - The barrier may be reusable depending on the backend implementation
  pub async fn wait(&self) {
    self.backend.wait().await;
  }

  /// Gets a reference to the backend implementation.
  ///
  /// This method is used when you need to access backend-specific functionality.
  /// For normal usage, the `new()` and `wait()` methods are sufficient.
  ///
  /// # Returns
  ///
  /// Immutable reference to the backend implementation
  ///
  /// # Examples
  ///
  /// ```ignore
  /// use nexus_utils_core_rs::AsyncBarrier;
  ///
  /// let barrier = AsyncBarrier::new(3);
  /// let backend = barrier.backend();
  /// // Execute backend-specific operations...
  /// ```
  pub fn backend(&self) -> &B {
    &self.backend
  }
}
