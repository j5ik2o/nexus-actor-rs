use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::future::FutureError;
use crate::actor::message::message_handle::{Message, MessageHandle};

#[derive(Debug, Clone)]
pub struct ReceiveTime;

#[derive(Debug, Clone)]
pub struct Restarting;

#[derive(Debug, Clone)]
pub struct Stopping;

#[derive(Debug, Clone)]
pub struct Stopped;

#[derive(Debug, Clone)]
pub struct Started;

#[derive(Debug, Clone)]
pub struct Restart {}

#[derive(Clone)]
pub struct ContinuationHandler(pub Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>);

impl ContinuationHandler {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    ContinuationHandler(Arc::new(move || Box::pin(f()) as BoxFuture<'static, ()>))
  }

  pub async fn run(&self) {
    (self.0)().await
  }
}

impl Debug for ContinuationHandler {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContinuationHandler")
  }
}

#[derive(Clone)]
pub(crate) struct Continuation {
  pub(crate) message: MessageHandle,
  pub(crate) f: ContinuationHandler,
}

impl Continuation {
  pub(crate) fn new<F, Fut>(message: MessageHandle, f: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Continuation {
      message,
      f: ContinuationHandler::new(move || Box::pin(f()) as BoxFuture<'static, ()>),
    }
  }
}

unsafe impl Send for Continuation {}
unsafe impl Sync for Continuation {}

impl Debug for Continuation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Continuation").field("message", &self.message).finish()
  }
}

impl Message for Continuation {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().is::<Continuation>()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

#[derive(Clone)]
pub struct ContinuationFunc(
  Arc<dyn Fn(Option<MessageHandle>, Option<FutureError>) -> BoxFuture<'static, ()> + Send + Sync + 'static>,
);

impl ContinuationFunc {
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(Option<MessageHandle>, Option<FutureError>) -> BoxFuture<'static, ()> + Send + Sync + 'static, {
    ContinuationFunc(Arc::new(f))
  }

  pub async fn run(&self, result: Option<MessageHandle>, error: Option<FutureError>) {
    (self.0)(result, error).await
  }
}

impl Debug for ContinuationFunc {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContinuationFunc")
  }
}
