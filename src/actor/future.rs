use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::BoxFuture;
use tokio::sync::{Mutex, Notify};
use tokio::time::timeout;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::DeadLetterResponse;
use crate::actor::actor_system::ActorSystem;
use crate::actor::log::P_LOG;
use crate::actor::message::{Message, MessageHandle};
use crate::actor::process::{Process, ProcessHandle};
use crate::log::field::Field;

#[derive(Debug, Clone)]
pub enum FutureError {
  Timeout,
  DeadLetter,
}

impl Message for FutureError {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().is::<FutureError>()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

impl std::error::Error for FutureError {}

impl std::fmt::Display for FutureError {
  fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    match self {
      FutureError::Timeout => write!(f, "future: timeout"),
      FutureError::DeadLetter => write!(f, "future: dead letter"),
    }
  }
}

#[derive(Debug, Clone)]
pub struct Future {
  inner: Arc<Mutex<FutureInner>>,
  notify: Arc<Notify>,
}

static_assertions::assert_impl_all!(Future: Send, Sync);

#[derive(Clone)]
struct CompletionFunc(Arc<dyn Fn(Option<MessageHandle>, Option<&FutureError>) -> BoxFuture<'static, ()> + Send>);

unsafe impl Send for CompletionFunc {}

unsafe impl Sync for CompletionFunc {}
impl CompletionFunc {
  fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(Option<MessageHandle>, Option<&FutureError>) -> Fut + Send + 'static,
    Fut: core::future::Future<Output = ()> + Send + 'static, {
    Self(Arc::new(move |message, error| {
      Box::pin(f(message, error)) as BoxFuture<'static, ()>
    }))
  }

  async fn run(&self, result: Option<MessageHandle>, error: Option<&FutureError>) {
    (self.0)(result, error).await
  }
}

impl Debug for CompletionFunc {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "CompletionFunc")
  }
}

#[derive(Debug)]
struct FutureInner {
  actor_system: ActorSystem,
  pid: Option<ExtendedPid>,
  done: bool,
  result: Option<MessageHandle>,
  error: Option<FutureError>,
  pipes: Vec<ExtendedPid>,
  completions: Vec<CompletionFunc>,
}

static_assertions::assert_impl_all!(FutureInner: Send, Sync);

#[derive(Debug)]
pub struct FutureProcess {
  future: Arc<Mutex<Future>>,
}

impl FutureProcess {
  pub async fn new(system: ActorSystem, duration: Duration) -> Arc<Self> {
    let inner = Arc::new(Mutex::new(FutureInner {
      actor_system: system.clone(),
      pid: None,
      done: false,
      result: None,
      error: None,
      pipes: Vec::new(),
      completions: Vec::new(),
    }));
    let notify = Arc::new(Notify::new());

    let future = Future { inner, notify };

    let future_process = Arc::new(FutureProcess {
      future: Arc::new(Mutex::new(future.clone())),
    });

    let process_registry = system.get_process_registry().await;
    let id = process_registry.next_id();

    let (pid, ok) = process_registry.add_process(
      ProcessHandle::new_arc(future_process.clone()),
      &format!("future_{}", id),
    );
    if !ok {
      P_LOG
        .error(
          "failed to register future process",
          vec![Field::stringer("pid", pid.to_string())],
        )
        .await;
    }
    future_process.set_pid(pid).await;

    if duration > Duration::from_secs(0) {
      let future_process_clone = Arc::clone(&future_process);

      tokio::spawn(async move {
        let future = future_process_clone.get_future().await;
        if timeout(duration, future.notify.notified()).await.is_err() {
          future_process_clone.handle_timeout().await;
        }
      });
    }

    future_process
  }

  pub async fn set_pid(&self, pid: ExtendedPid) {
    let mut future = self.future.lock().await.clone();
    future.set_pid(pid).await;
  }

  pub async fn get_pid(&self) -> ExtendedPid {
    let future = self.future.lock().await.clone();
    future.get_pid().await.clone()
  }

  pub async fn get_future(&self) -> Future {
    let future = self.future.lock().await.clone();
    future.clone()
  }

  pub async fn is_empty(&self) -> bool {
    let future_mg = self.future.lock().await.clone();
    let inner = future_mg.inner.lock().await;
    !inner.done
  }

  pub async fn pipe_to(&self, pid: ExtendedPid) {
    let future_mg = self.future.lock().await.clone();
    future_mg.pipe_to(pid).await;
  }

  pub async fn result(&self) -> Result<MessageHandle, FutureError> {
    let future_mg = self.future.lock().await.clone();
    future_mg.result().await
  }

  pub async fn complete(&self, result: MessageHandle) {
    let future_mg = self.future.lock().await.clone();
    future_mg.complete(result).await;
  }

  pub async fn fail(&self, error: FutureError) {
    let future_mg = self.future.lock().await.clone();
    future_mg.fail(error).await;
  }

  async fn handle_timeout(&self) {
    let error = FutureError::Timeout;
    {
      let future_mg = self.future.lock().await;
      future_mg.fail(error.clone()).await;
    }

    let future = self.future.lock().await.clone();
    let mut inner = future.inner.lock().await;
    for pipe in &inner.pipes {
      pipe
        .send_user_message(inner.actor_system.clone(), MessageHandle::new(error.clone()))
        .await;
    }
    inner.pipes.clear();
  }
}

#[async_trait]
impl Process for FutureProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, message: MessageHandle) {
    let future = self.future.lock().await.clone();
    tokio::spawn({
      let future = future.clone();
      async move {
        if message.as_any().downcast_ref::<DeadLetterResponse>().is_some() {
          future.fail(FutureError::DeadLetter).await;
        } else {
          future.complete(message).await;
        }
        future.instrument().await;
      }
    });
  }

  async fn send_system_message(&self, _pid: &ExtendedPid, message: MessageHandle) {
    let future = self.future.lock().await.clone();
    tokio::spawn({
      let future = future.clone();
      async move {
        future.complete(message).await;
        future.instrument().await;
      }
    });
  }

  async fn stop(&self, _pid: &ExtendedPid) {}

  fn set_dead(&self) {}

  fn as_any(&self) -> &dyn Any {
    self
  }
}

impl Future {
  pub async fn result(&self) -> Result<MessageHandle, FutureError> {
    loop {
      {
        let inner = self.inner.lock().await;
        if inner.done {
          return if let Some(error) = &inner.error {
            Err(error.clone())
          } else {
            Ok(inner.result.as_ref().unwrap().clone())
          };
        }
      }
      self.notify.notified().await;
    }
  }

  pub async fn set_pid(&mut self, pid: ExtendedPid) {
    let mut inner = self.inner.lock().await;
    inner.pid = Some(pid);
  }

  pub async fn get_pid(&self) -> ExtendedPid {
    let inner = self.inner.lock().await;
    inner.pid.clone().unwrap()
  }

  pub async fn pipe_to(&self, pid: ExtendedPid) {
    let mut inner = self.inner.lock().await;
    inner.pipes.push(pid);
    if inner.done {
      self.send_to_pipes(&mut inner).await;
    }
  }

  async fn send_to_pipes(&self, inner: &mut FutureInner) {
    let message = if let Some(error) = &inner.error {
      MessageHandle::new(error.clone())
    } else {
      inner.result.as_ref().unwrap().clone()
    };

    for process in &inner.pipes {
      process
        .send_user_message(inner.actor_system.clone(), message.clone())
        .await;
    }

    inner.pipes.clear();
  }

  pub async fn complete(&self, result: MessageHandle) {
    let mut inner = self.inner.lock().await;
    if !inner.done {
      inner.result = Some(result);
      inner.done = true;
      self.send_to_pipes(&mut inner).await;
      self.run_completions(&mut inner).await;
      self.notify.notify_waiters();
    }
  }

  pub async fn fail(&self, error: FutureError) {
    let mut inner = self.inner.lock().await;
    if !inner.done {
      inner.error = Some(error);
      inner.done = true;
      self.send_to_pipes(&mut inner).await;
      self.run_completions(&mut inner).await;
      self.notify.notify_waiters();
    }
  }

  pub async fn continue_with<F, Fut>(&self, continuation: F)
  where
    F: Fn(Option<MessageHandle>, Option<&FutureError>) -> Fut + Send + Sync + 'static,
    Fut: core::future::Future<Output = ()> + Send + 'static, {
    let mut inner = self.inner.lock().await;
    if inner.done {
      continuation(inner.result.clone(), inner.error.as_ref()).await;
    } else {
      inner.completions.push(CompletionFunc::new(continuation));
    }
  }

  async fn run_completions(&self, inner: &mut FutureInner) {
    for completion in inner.completions.drain(..) {
      completion.run(inner.result.clone(), inner.error.as_ref()).await;
    }
  }

  async fn instrument(&self) {
    // Here you would implement your metrics logging
    // This is a placeholder for the actual implementation
    println!("Future completed");
  }
}
