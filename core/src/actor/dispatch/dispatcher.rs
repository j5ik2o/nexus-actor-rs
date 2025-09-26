//! Dispatcher implementations and handles.
//!
//! Runtime管理の方針については `docs/dispatcher_runtime_policy.md` を参照。

use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use tokio::runtime::{Builder, Runtime};

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
}

// --- TokioRuntimeContextDispatcher implementation

#[derive(Debug, Clone)]
pub struct TokioRuntimeContextDispatcher {
  throughput: i32,
}

impl TokioRuntimeContextDispatcher {
  pub fn new() -> Result<Self, std::io::Error> {
    Ok(Self { throughput: 300 })
  }

  pub fn with_throughput(mut self, throughput: i32) -> Self {
    self.throughput = throughput;
    self
  }
}

#[async_trait]
impl Dispatcher for TokioRuntimeContextDispatcher {
  async fn schedule(&self, runner: Runnable) {
    tokio::spawn(runner.run());
  }

  async fn throughput(&self) -> i32 {
    self.throughput
  }
}

// --- TokioRuntimeDispatcher implementation

#[derive(Debug, Clone)]
pub struct TokioRuntimeDispatcher {
  runtime: Arc<Runtime>,
  throughput: i32,
}

impl TokioRuntimeDispatcher {
  pub fn new() -> Result<Self, std::io::Error> {
    match Runtime::new() {
      Ok(runtime) => Ok(Self {
        runtime: Arc::new(runtime),
        throughput: 300,
      }),
      Err(e) => Err(e),
    }
  }

  pub fn with_runtime(mut self, runtime: Runtime) -> Self {
    self.runtime = Arc::new(runtime);
    self
  }

  pub fn with_throughput(mut self, throughput: i32) -> Self {
    self.throughput = throughput;
    self
  }
}

#[async_trait]
impl Dispatcher for TokioRuntimeDispatcher {
  async fn schedule(&self, runner: Runnable) {
    self.runtime.spawn(runner.run());
  }

  async fn throughput(&self) -> i32 {
    self.throughput
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
/// use nexus_actor_core_rs::actor::dispatch::{Dispatcher, SingleWorkerDispatcher, Runnable};
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
  runtime: Option<Arc<Runtime>>,
  throughput: i32,
}

impl SingleWorkerDispatcher {
  pub fn new() -> Result<Self, std::io::Error> {
    let runtime = Builder::new_multi_thread().worker_threads(1).enable_all().build()?;
    Ok(Self {
      runtime: Some(Arc::new(runtime)),
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
