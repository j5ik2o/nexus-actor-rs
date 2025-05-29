use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::context::ReceiverContextHandle;
use crate::actor::core::actor_error::ActorError;
use crate::actor::message::MessageEnvelope;

type ReceiverMiddlewareFn = Arc<dyn Fn(ReceiverContextHandle, MessageEnvelope) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync>;

// ReceiverMiddlewareChain
#[derive(Clone)]
pub struct ReceiverMiddlewareChain(ReceiverMiddlewareFn);

unsafe impl Send for ReceiverMiddlewareChain {}
unsafe impl Sync for ReceiverMiddlewareChain {}

impl ReceiverMiddlewareChain {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ReceiverContextHandle, MessageEnvelope) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    Self(Arc::new(move |rch, me| {
      Box::pin(f(rch, me)) as BoxFuture<'static, Result<(), ActorError>>
    }))
  }

  pub async fn run(&self, context: ReceiverContextHandle, envelope: MessageEnvelope) -> Result<(), ActorError> {
    self.0(context, envelope).await
  }
}

impl Debug for ReceiverMiddlewareChain {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ReceiverMiddlewareChain")
  }
}

impl PartialEq for ReceiverMiddlewareChain {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ReceiverMiddlewareChain {}

impl std::hash::Hash for ReceiverMiddlewareChain {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref()
      as *const dyn Fn(ReceiverContextHandle, MessageEnvelope) -> BoxFuture<'static, Result<(), ActorError>>)
      .hash(state);
  }
}

static_assertions::assert_impl_all!(ReceiverMiddlewareChain: Send, Sync);
