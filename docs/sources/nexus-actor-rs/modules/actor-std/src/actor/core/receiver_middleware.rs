use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use futures::future::BoxFuture;
use nexus_actor_core_rs::context::CoreReceiverMiddleware;

use crate::actor::context::ReceiverSnapshot;
use crate::actor::core::actor_error::ActorError;
use crate::actor::core::receiver_middleware_chain::ReceiverMiddlewareChain;

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct ReceiverMiddleware(CoreReceiverMiddleware<ReceiverSnapshot, ActorError>);

unsafe impl Send for ReceiverMiddleware {}
unsafe impl Sync for ReceiverMiddleware {}

impl ReceiverMiddleware {
  pub fn new(f: impl Fn(ReceiverMiddlewareChain) -> ReceiverMiddlewareChain + Send + Sync + 'static) -> Self {
    let middleware = CoreReceiverMiddleware::new(move |chain| {
      let wrapped = ReceiverMiddlewareChain::from_core(chain);
      let result = f(wrapped);
      result.into_core()
    });
    ReceiverMiddleware(middleware)
  }

  pub fn from_sync(f: impl Fn(ReceiverSnapshot) -> ReceiverSnapshot + Send + Sync + 'static) -> Self {
    let arc = Arc::new(f);
    ReceiverMiddleware::new(move |chain| {
      let arc = arc.clone();
      chain.with_sync(move |snapshot| arc(snapshot))
    })
  }

  pub fn from_async(
    f: impl Fn(
        ReceiverSnapshot,
        Arc<dyn Fn(ReceiverSnapshot) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync>,
      ) -> BoxFuture<'static, Result<(), ActorError>>
      + Send
      + Sync
      + 'static,
  ) -> Self {
    let arc = Arc::new(f);
    ReceiverMiddleware::new(move |chain| {
      let arc = arc.clone();
      chain.with_async(move |snapshot, next| arc(snapshot, next))
    })
  }

  pub fn run(&self, next: ReceiverMiddlewareChain) -> ReceiverMiddlewareChain {
    let result = self.0.run(next.into_core());
    ReceiverMiddlewareChain::from_core(result)
  }

  pub fn as_core(&self) -> &CoreReceiverMiddleware<ReceiverSnapshot, ActorError> {
    &self.0
  }

  pub fn into_core(self) -> CoreReceiverMiddleware<ReceiverSnapshot, ActorError> {
    self.0
  }
}

impl Debug for ReceiverMiddleware {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ReceiverMiddleware")
  }
}

impl PartialEq for ReceiverMiddleware {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl Eq for ReceiverMiddleware {}

impl std::hash::Hash for ReceiverMiddleware {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.0.hash(state);
  }
}

static_assertions::assert_impl_all!(ReceiverMiddleware: Send, Sync);
