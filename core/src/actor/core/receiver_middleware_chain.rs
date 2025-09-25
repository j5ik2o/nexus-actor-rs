use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::context::ReceiverContextHandle;
use crate::actor::core::actor_error::ActorError;
use crate::actor::message::MessageEnvelope;

type ReceiverMiddlewareFn =
  Arc<dyn Fn(ReceiverContextHandle, MessageEnvelope) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync>;

// ReceiverMiddlewareChain
#[derive(Clone)]
pub struct ReceiverMiddlewareChain(ReceiverMiddlewareFn);

unsafe impl Send for ReceiverMiddlewareChain {}
unsafe impl Sync for ReceiverMiddlewareChain {}

impl ReceiverMiddlewareChain {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ReceiverContextHandle, MessageEnvelope) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static,
  {
    Self(Arc::new(move |rch, me| {
      Box::pin(f(rch, me)) as BoxFuture<'static, Result<(), ActorError>>
    }))
  }

  pub async fn run(&self, context: ReceiverContextHandle, envelope: MessageEnvelope) -> Result<(), ActorError> {
    let stats_before = context.context_cell_stats();
    let result = (self.0)(context.clone(), envelope).await;
    let stats_after = context.context_cell_stats();

    let hits_delta = stats_after.hits.saturating_sub(stats_before.hits);
    let misses_delta = stats_after.misses.saturating_sub(stats_before.misses);
    let delta_total = hits_delta + misses_delta;

    if delta_total > 0 {
      let hit_rate = hits_delta as f64 / delta_total as f64;
      let actor_self = context
        .actor_context_arc()
        .and_then(|ctx| ctx.borrow().self_pid().cloned());
      tracing::debug!(
        target = "nexus::actor::context_cell",
        hits_total = stats_after.hits,
        misses_total = stats_after.misses,
        hits_delta,
        misses_delta,
        delta_total,
        hit_rate,
        error = result.is_err(),
        ?actor_self,
        "receiver middleware chain context cell stats delta"
      );
    }

    result
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
