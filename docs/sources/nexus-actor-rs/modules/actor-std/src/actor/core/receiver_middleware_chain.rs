use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;

use futures::future::BoxFuture;
use nexus_actor_core_rs::actor::core_types::actor_error::CoreActorError;
use nexus_actor_core_rs::context::{CoreReceiverInvocation, CoreReceiverMiddlewareChain};

use crate::actor::context::{ReceiverContextHandle, ReceiverSnapshot};
use crate::actor::core::actor_error::ActorError;
use crate::actor::message::MessageEnvelope;

type ReceiverAsyncFn = Arc<dyn Fn(ReceiverSnapshot) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync>;

#[derive(Clone)]
pub struct ReceiverMiddlewareChain {
  inner: CoreReceiverMiddlewareChain<ReceiverSnapshot, ActorError>,
}

unsafe impl Send for ReceiverMiddlewareChain {}
unsafe impl Sync for ReceiverMiddlewareChain {}

impl ReceiverMiddlewareChain {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ReceiverContextHandle, MessageEnvelope) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    let inner = CoreReceiverMiddlewareChain::new(move |snapshot: ReceiverSnapshot| {
      let (context_snapshot, message) = snapshot.into_parts();
      let context_handle = context_snapshot
        .into_context_handle()
        .expect("ContextSnapshot missing ContextHandle in receiver middleware tail");
      Box::pin(f(ReceiverContextHandle::new(context_handle), message))
    });

    Self { inner }
  }

  pub fn with_sync<F>(self, sync: F) -> Self
  where
    F: Fn(ReceiverSnapshot) -> ReceiverSnapshot + Send + Sync + 'static, {
    Self {
      inner: self.inner.with_sync(sync),
    }
  }

  pub fn with_async<F>(self, wrapper: F) -> Self
  where
    F: Fn(ReceiverSnapshot, ReceiverAsyncFn) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync + 'static, {
    let inner = self.inner.with_async(move |snapshot, next| {
      let next_arc: ReceiverAsyncFn = {
        let next = next.clone();
        Arc::new(move |snapshot: ReceiverSnapshot| {
          let fut = next(snapshot);
          fut
        })
      };
      wrapper(snapshot, next_arc)
    });

    Self { inner }
  }

  pub fn from_core(inner: CoreReceiverMiddlewareChain<ReceiverSnapshot, ActorError>) -> Self {
    Self { inner }
  }

  pub fn into_core(self) -> CoreReceiverMiddlewareChain<ReceiverSnapshot, ActorError> {
    self.inner
  }

  pub fn as_core(&self) -> &CoreReceiverMiddlewareChain<ReceiverSnapshot, ActorError> {
    &self.inner
  }

  pub fn to_core_invocation_chain(&self) -> CoreReceiverMiddlewareChain<CoreReceiverInvocation, CoreActorError> {
    let inner = self.inner.clone();
    CoreReceiverMiddlewareChain::new(move |invocation: CoreReceiverInvocation| {
      let inner = inner.clone();
      async move {
        let snapshot = ReceiverSnapshot::from_core_invocation(invocation);
        let transformed = inner.apply_sync(snapshot);
        inner.call_async(transformed).await.map_err(CoreActorError::from)
      }
    })
  }

  pub async fn run(&self, context: ReceiverContextHandle, envelope: MessageEnvelope) -> Result<(), ActorError> {
    let stats_before = context.context_cell_stats();
    let context_handle = context.context_handle().clone();
    let snapshot_with_core = context_handle.snapshot_with_core().await;
    let snapshot = ReceiverSnapshot::new(snapshot_with_core, envelope);
    let transformed = self.inner.apply_sync(snapshot);
    let result = self.inner.call_async(transformed).await;
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
    self.inner == other.inner
  }
}

impl Eq for ReceiverMiddlewareChain {}

impl Hash for ReceiverMiddlewareChain {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.inner.hash(state);
  }
}

static_assertions::assert_impl_all!(ReceiverMiddlewareChain: Send, Sync);
