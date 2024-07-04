use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::message::message_handle::{Message, MessageHandle};

#[derive(Clone)]
pub(crate) struct Continuation {
  pub(crate) message: MessageHandle,
  pub(crate) f: ContinuationCallbackFunc,
}

impl Continuation {
  pub(crate) fn new<F, Fut>(message: MessageHandle, f: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Continuation {
      message,
      f: ContinuationCallbackFunc::new(move || Box::pin(f()) as BoxFuture<'static, ()>),
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
    if let Some(other) = other.as_any().downcast_ref::<Self>() {
      self.message == other.message
    } else {
      false
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

#[derive(Clone)]
pub struct ContinuationCallbackFunc(pub Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>);

impl ContinuationCallbackFunc {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Self(Arc::new(move || Box::pin(f()) as BoxFuture<'static, ()>))
  }

  pub async fn run(&self) {
    (self.0)().await
  }
}

impl Debug for ContinuationCallbackFunc {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContinuationHandler")
  }
}
