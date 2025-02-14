use std::any::Any;
use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::Message;

#[derive(Clone)]
pub(crate) struct Continuation {
  pub(crate) message_handle: MessageHandle,
  pub(crate) f: ContinuationCallback,
}

impl Continuation {
  pub(crate) fn new<F, Fut>(message_handle: MessageHandle, f: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Continuation {
      message_handle,
      f: ContinuationCallback::new(move || Box::pin(f()) as BoxFuture<'static, ()>),
    }
  }
}

static_assertions::assert_impl_all!(Continuation: Send, Sync);

impl Debug for Continuation {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Continuation")
      .field("message", &self.message_handle)
      .finish()
  }
}

impl Message for Continuation {
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other) = other.as_any().downcast_ref::<Self>() {
      self.message_handle == other.message_handle
    } else {
      false
    }
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn message_type(&self) -> &'static str {
    "Continuation"
  }
}

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct ContinuationCallback(Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync + 'static>);

unsafe impl Send for ContinuationCallback {}
unsafe impl Sync for ContinuationCallback {}

impl ContinuationCallback {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Self(Arc::new(move || Box::pin(f()) as BoxFuture<'static, ()>))
  }

  pub async fn run(&self) {
    tracing::debug!("ContinuationCallback::run: start");
    (self.0)().await;
    tracing::debug!("ContinuationCallback::run: end");
  }
}

impl Debug for ContinuationCallback {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContinuationHandler")
  }
}
