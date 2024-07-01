use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::DeadLetterResponse;
use crate::actor::message::{Message, MessageHandle};
use crate::actor::process::Process;
use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify};
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub enum FutureError {
  Timeout,
  DeadLetter,
}

impl Message for FutureError {
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

struct CompletionFunc(Box<dyn Fn(Option<MessageHandle>, Option<&FutureError>) + Send>);

impl CompletionFunc {
  fn new<F>(f: F) -> Self
  where
    F: Fn(Option<MessageHandle>, Option<&FutureError>) + Send + 'static, {
    Self(Box::new(f))
  }

  fn run(&self, result: Option<MessageHandle>, error: Option<&FutureError>) {
    (self.0)(result, error);
  }
}

impl Debug for CompletionFunc {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "CompletionFunc")
  }
}

#[derive(Debug)]
struct FutureInner {
  done: bool,
  result: Option<MessageHandle>,
  error: Option<FutureError>,
  pipes: Vec<Arc<dyn Process>>,
  completions: Vec<CompletionFunc>,
}

#[derive(Debug)]
pub struct FutureProcess {
  future: Future,
}

impl FutureProcess {
  pub fn new(duration: Duration) -> Arc<Self> {
    let inner = Arc::new(Mutex::new(FutureInner {
      done: false,
      result: None,
      error: None,
      pipes: Vec::new(),
      completions: Vec::new(),
    }));
    let notify = Arc::new(Notify::new());

    let future = Future { inner, notify };

    let future_process = Arc::new(FutureProcess { future: future.clone() });

    if duration > Duration::from_secs(0) {
      let future_process_clone = Arc::clone(&future_process);

      tokio::spawn(async move {
        if timeout(duration, future_process_clone.future.notify.notified())
          .await
          .is_err()
        {
          future_process_clone.handle_timeout().await;
        }
      });
    }

    future_process
  }

  pub async fn is_empty(&self) -> bool {
    let inner = self.future.inner.lock().await;
    !inner.done
  }

  pub async fn pipe_to(&self, process: Arc<dyn Process>) {
    self.future.pipe_to(process).await;
  }

  pub async fn result(&self) -> Result<MessageHandle, FutureError> {
    self.future.result().await
  }

  pub async fn complete(&self, result: MessageHandle) {
    self.future.complete(result).await;
  }

  pub async fn fail(&self, error: FutureError) {
    self.future.fail(error).await;
  }

  async fn handle_timeout(&self) {
    let error = FutureError::Timeout;
    self.future.fail(error.clone()).await;

    let mut inner = self.future.inner.lock().await;
    for pipe in &inner.pipes {
      pipe.send_user_message(None, MessageHandle::new(error.clone())).await;
    }
    inner.pipes.clear();
  }
}

#[async_trait]
impl Process for FutureProcess {
  async fn send_user_message(&self, _: Option<&ExtendedPid>, message: MessageHandle) {
    let future = self.future.clone();
    tokio::spawn(async move {
      if message.as_any().downcast_ref::<DeadLetterResponse>().is_some() {
        future.fail(FutureError::DeadLetter).await;
      } else {
        future.complete(message).await;
      }
      future.instrument().await;
    });
  }

  async fn send_system_message(&self, _pid: &ExtendedPid, message: MessageHandle) {
    let future = self.future.clone();
    tokio::spawn(async move {
      future.complete(message).await;
      future.instrument().await;
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

  pub async fn pipe_to(&self, process: Arc<dyn Process>) {
    let mut inner = self.inner.lock().await;
    inner.pipes.push(process);
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
      process.send_user_message(None, message.clone()).await;
    }

    inner.pipes.clear();
  }

  pub async fn complete(&self, result: MessageHandle) {
    let mut inner = self.inner.lock().await;
    if !inner.done {
      inner.result = Some(result);
      inner.done = true;
      self.send_to_pipes(&mut inner).await;
      self.run_completions(&mut inner);
      self.notify.notify_waiters();
    }
  }

  pub async fn fail(&self, error: FutureError) {
    let mut inner = self.inner.lock().await;
    if !inner.done {
      inner.error = Some(error);
      inner.done = true;
      self.send_to_pipes(&mut inner).await;
      self.run_completions(&mut inner);
      self.notify.notify_waiters();
    }
  }

  pub async fn continue_with<F>(&self, continuation: F)
  where
    F: Fn(Option<MessageHandle>, Option<&FutureError>) + Send + 'static, {
    let mut inner = self.inner.lock().await;
    if inner.done {
      continuation(inner.result.clone(), inner.error.as_ref());
    } else {
      inner.completions.push(CompletionFunc::new(continuation));
    }
  }

  fn run_completions(&self, inner: &mut FutureInner) {
    for completion in inner.completions.drain(..) {
      completion.run(inner.result.clone(), inner.error.as_ref());
    }
  }

  async fn instrument(&self) {
    // Here you would implement your metrics logging
    // This is a placeholder for the actual implementation
    println!("Future completed");
  }

  pub fn clone(&self) -> Self {
    Future {
      inner: Arc::clone(&self.inner),
      notify: Arc::clone(&self.notify),
    }
  }
}
