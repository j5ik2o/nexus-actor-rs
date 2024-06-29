use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use tokio::runtime::{Builder, Runtime};
use tokio::sync::Mutex;

use crate::actor::message::{Message, MessageHandle};
use crate::actor::{Reason, ReasonHandle};

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
  pub fn new(throughput: i32) -> Result<Self, std::io::Error> {
    Ok(Self { throughput })
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
  pub fn new(runtime: Runtime, throughput: i32) -> Self {
    Self {
      runtime: Arc::new(runtime),
      throughput,
    }
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

#[derive(Debug, Clone)]
pub struct SingleWorkerDispatcher {
  runtime: Arc<Runtime>,
}

impl SingleWorkerDispatcher {
  pub fn new() -> Result<Self, std::io::Error> {
    let runtime = Builder::new_multi_thread().worker_threads(1).enable_all().build()?;
    Ok(Self {
      runtime: Arc::new(runtime),
    })
  }
}

#[async_trait]
impl Dispatcher for SingleWorkerDispatcher {
  async fn schedule(&self, runner: Runnable) {
    self.runtime.spawn(runner.run());
  }

  async fn throughput(&self) -> i32 {
    1
  }
}

// --- CurrentThreadDispatcher implementation

#[derive(Debug, Clone)]
pub struct CurrentThreadDispatcher {
  runtime: Arc<Runtime>,
  throughput: i32,
}

impl CurrentThreadDispatcher {
  pub fn new(throughput: i32) -> Result<Self, std::io::Error> {
    let runtime = Builder::new_current_thread().enable_all().build()?;
    Ok(Self {
      runtime: Arc::new(runtime),
      throughput,
    })
  }
}

#[async_trait]
impl Dispatcher for CurrentThreadDispatcher {
  async fn schedule(&self, runner: Runnable) {
    self.runtime.spawn(runner.run());
  }

  async fn throughput(&self) -> i32 {
    self.throughput
  }
}
