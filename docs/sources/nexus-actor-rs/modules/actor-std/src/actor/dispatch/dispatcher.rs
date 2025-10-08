//! Dispatcher implementations and handles.
//!
//! Runtime管理の方針については `docs/dispatcher_runtime_policy.md` を参照。

use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use crate::runtime::build_single_worker_runtime;
use async_trait::async_trait;
use futures::future::BoxFuture;

use crate::runtime::StdAsyncMutex;
use nexus_actor_core_rs::runtime::AsyncMutex;
use nexus_actor_core_rs::runtime::{CoreRuntime, CoreScheduledTask, CoreScheduler};

#[cfg(test)]
mod tests;

pub struct Runnable(Box<dyn FnOnce() -> BoxFuture<'static, ()> + Send + 'static>);

impl Runnable {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Self(Box::new(move || Box::pin(f()) as BoxFuture<'static, ()>))
  }

  pub async fn run(self) {
    (self.0)().await;
  }
}

// Dispatcher trait
#[async_trait]
pub trait Dispatcher: Debug + Send + Sync + 'static {
  async fn schedule(&self, runner: Runnable);
  async fn throughput(&self) -> i32;

  fn yield_hint(&self) -> bool {
    false
  }
}

// --- CoreSchedulerDispatcher implementation

#[derive(Clone)]
pub struct CoreSchedulerDispatcher {
  scheduler: Arc<dyn CoreScheduler>,
  throughput: i32,
}

impl std::fmt::Debug for CoreSchedulerDispatcher {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("CoreSchedulerDispatcher")
      .field("throughput", &self.throughput)
      .finish()
  }
}

impl CoreSchedulerDispatcher {
  pub fn from_runtime(runtime: CoreRuntime) -> Self {
    Self::from_scheduler(runtime.scheduler())
  }

  pub fn from_scheduler(scheduler: Arc<dyn CoreScheduler>) -> Self {
    Self {
      scheduler,
      throughput: 300,
    }
  }

  pub fn with_throughput(mut self, throughput: i32) -> Self {
    self.throughput = throughput;
    self
  }
}

#[async_trait]
impl Dispatcher for CoreSchedulerDispatcher {
  async fn schedule(&self, runner: Runnable) {
    let runner_cell = Arc::new(StdAsyncMutex::new(Some(runner)));
    let task: CoreScheduledTask = Arc::new(move || {
      let runner_cell = runner_cell.clone();
      Box::pin(async move {
        if let Some(runner) = runner_cell.lock().await.take() {
          runner.run().await;
        }
      })
    });

    let _ = self.scheduler.schedule_once(Duration::ZERO, task);
  }

  async fn throughput(&self) -> i32 {
    self.throughput
  }
}

#[derive(Debug, Clone)]
pub struct DispatcherHandle(Arc<dyn Dispatcher>);

impl DispatcherHandle {
  pub fn new_arc(dispatcher: Arc<dyn Dispatcher>) -> Self {
    Self(dispatcher)
  }

  pub fn new(dispatcher: impl Dispatcher + 'static) -> Self {
    Self(Arc::new(dispatcher))
  }
}

#[async_trait]
impl Dispatcher for DispatcherHandle {
  async fn schedule(&self, runner: Runnable) {
    self.0.schedule(runner).await;
  }

  async fn throughput(&self) -> i32 {
    self.0.throughput().await
  }

  fn yield_hint(&self) -> bool {
    self.0.yield_hint()
  }
}

// --- SingleWorkerDispatcher implementation

/// Dispatcher that executes work on a dedicated Tokio runtime.
///
/// ## Runtime lifecycle
/// The internal runtime is owned via `Option<Arc<Runtime>>`.
/// When this dispatcher is dropped, it will call `shutdown_background()`
/// on the runtime if this instance is the last owner. This mirrors the
/// policy described in `docs/dispatcher_runtime_policy.md`.
///
/// ```rust
/// use nexus_actor_std_rs::actor::dispatch::{Dispatcher, SingleWorkerDispatcher, Runnable};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let dispatcher = SingleWorkerDispatcher::new()?;
/// dispatcher.schedule(Runnable::new(|| async move {
///   // async work
/// })).await;
/// // When `dispatcher` is dropped, the internal runtime is shut down safely.
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SingleWorkerDispatcher {
  runtime: Option<Arc<tokio::runtime::Runtime>>,
  throughput: i32,
}

impl SingleWorkerDispatcher {
  pub fn new() -> Result<Self, std::io::Error> {
    let factory = build_single_worker_runtime()?;
    Ok(Self {
      runtime: Some(runtime),
      throughput: 300,
    })
  }

  pub fn with_throughput(mut self, throughput: i32) -> Self {
    self.throughput = throughput;
    self
  }
}

#[async_trait]
impl Dispatcher for SingleWorkerDispatcher {
  async fn schedule(&self, runner: Runnable) {
    if let Some(runtime) = &self.runtime {
      runtime.spawn(runner.run());
    } else {
      tracing::warn!("SingleWorkerDispatcher runtime already shut down");
    }
  }

  async fn throughput(&self) -> i32 {
    self.throughput
  }
}

impl Drop for SingleWorkerDispatcher {
  fn drop(&mut self) {
    if let Some(runtime_arc) = self.runtime.take() {
      if Arc::strong_count(&runtime_arc) == 1 {
        if let Ok(runtime) = Arc::try_unwrap(runtime_arc) {
          runtime.shutdown_background();
        }
      }
    }
  }
}

// --- CurrentThreadDispatcher implementation

#[derive(Debug, Clone)]
pub struct CurrentThreadDispatcher {
  throughput: i32,
}

impl CurrentThreadDispatcher {
  pub fn new() -> Result<Self, std::io::Error> {
    Ok(Self { throughput: 300 })
  }

  pub fn with_throughput(mut self, throughput: i32) -> Self {
    self.throughput = throughput;
    self
  }
}

#[async_trait]
impl Dispatcher for CurrentThreadDispatcher {
  async fn schedule(&self, runner: Runnable) {
    runner.run().await
  }

  async fn throughput(&self) -> i32 {
    self.throughput
  }
}
