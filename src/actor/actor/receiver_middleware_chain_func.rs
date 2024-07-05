use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;
use crate::actor::actor::actor_error::ActorError;
use crate::actor::context::receiver_context_handle::ReceiverContextHandle;
use crate::actor::message::message_or_envelope::MessageEnvelope;

// ReceiverFunc
#[derive(Clone)]
pub struct ReceiverMiddlewareChainFunc(
  Arc<dyn Fn(ReceiverContextHandle, MessageEnvelope) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync>,
);

unsafe impl Send for ReceiverMiddlewareChainFunc {}
unsafe impl Sync for ReceiverMiddlewareChainFunc {}

impl ReceiverMiddlewareChainFunc {
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

impl Debug for ReceiverMiddlewareChainFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ReceiverFunc")
  }
}

impl PartialEq for ReceiverMiddlewareChainFunc {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ReceiverMiddlewareChainFunc {}

impl std::hash::Hash for ReceiverMiddlewareChainFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref()
      as *const dyn Fn(ReceiverContextHandle, MessageEnvelope) -> BoxFuture<'static, Result<(), ActorError>>)
      .hash(state);
  }
}


