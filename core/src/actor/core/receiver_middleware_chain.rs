use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::context::{ReceiverContextHandle, ReceiverSnapshot};
use crate::actor::core::actor_error::ActorError;
use crate::actor::message::MessageEnvelope;

type ReceiverSyncFn = Arc<dyn Fn(ReceiverSnapshot) -> ReceiverSnapshot + Send + Sync + 'static>;
type ReceiverAsyncFn = Arc<dyn Fn(ReceiverSnapshot) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync>;

#[derive(Clone)]
pub struct ReceiverMiddlewareChain {
  sync_step: ReceiverSyncFn,
  async_step: ReceiverAsyncFn,
}

unsafe impl Send for ReceiverMiddlewareChain {}
unsafe impl Sync for ReceiverMiddlewareChain {}

impl ReceiverMiddlewareChain {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ReceiverContextHandle, MessageEnvelope) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    let async_step: ReceiverAsyncFn = Arc::new(move |snapshot: ReceiverSnapshot| {
      let (context_snapshot, message) = snapshot.into_parts();
      let context_handle = context_snapshot
        .context_handle()
        .cloned()
        .expect("ContextSnapshot missing ContextHandle in receiver middleware tail");
      Box::pin(f(ReceiverContextHandle::new(context_handle), message))
    });

    Self {
      sync_step: Arc::new(|snapshot| snapshot),
      async_step,
    }
  }

  pub fn with_sync<F>(self, sync: F) -> Self
  where
    F: Fn(ReceiverSnapshot) -> ReceiverSnapshot + Send + Sync + 'static, {
    let previous = self.sync_step.clone();
    Self {
      sync_step: Arc::new(move |snapshot| {
        let after_prev = previous(snapshot);
        sync(after_prev)
      }),
      async_step: self.async_step.clone(),
    }
  }

  pub fn with_async<F>(self, wrapper: F) -> Self
  where
    F: Fn(ReceiverSnapshot, ReceiverAsyncFn) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync + 'static, {
    let async_prev = self.async_step.clone();
    Self {
      sync_step: self.sync_step.clone(),
      async_step: Arc::new(move |snapshot| wrapper(snapshot, async_prev.clone())),
    }
  }

  pub async fn run(&self, context: ReceiverContextHandle, envelope: MessageEnvelope) -> Result<(), ActorError> {
    let stats_before = context.context_cell_stats();
    let snapshot = ReceiverSnapshot::new(context.context_handle().snapshot_with_borrow(), envelope);
    let transformed = (self.sync_step)(snapshot);
    let result = (self.async_step)(transformed).await;
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
    Arc::ptr_eq(&self.sync_step, &other.sync_step) && Arc::ptr_eq(&self.async_step, &other.async_step)
  }
}

impl Eq for ReceiverMiddlewareChain {}

impl std::hash::Hash for ReceiverMiddlewareChain {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (Arc::as_ptr(&self.sync_step) as *const ()).hash(state);
    (Arc::as_ptr(&self.async_step) as *const ()).hash(state);
  }
}

static_assertions::assert_impl_all!(ReceiverMiddlewareChain: Send, Sync);
