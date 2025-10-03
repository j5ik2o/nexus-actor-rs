use std::fmt::{Debug, Formatter};
use std::future::Future;

use futures::future::BoxFuture;
use nexus_actor_core_rs::context::CoreSenderMiddlewareChain;
use nexus_actor_core_rs::CorePid;

use crate::actor::context::SenderContextHandle;
use crate::actor::core::pid::ExtendedPid;
use crate::actor::message::MessageEnvelope;

pub type SenderInvocation = (SenderContextHandle, CorePid, MessageEnvelope);

#[derive(Clone)]
pub struct SenderMiddlewareChain {
  inner: CoreSenderMiddlewareChain<SenderInvocation>,
}

impl Debug for SenderMiddlewareChain {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SenderMiddlewareChain")
  }
}

impl PartialEq for SenderMiddlewareChain {
  fn eq(&self, other: &Self) -> bool {
    self.inner == other.inner
  }
}

impl Eq for SenderMiddlewareChain {}

impl std::hash::Hash for SenderMiddlewareChain {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.inner.hash(state);
  }
}

impl SenderMiddlewareChain {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(SenderContextHandle, CorePid, MessageEnvelope) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    let inner = CoreSenderMiddlewareChain::new(move |(context, target, envelope): SenderInvocation| {
      Box::pin(f(context, target, envelope)) as BoxFuture<'static, ()>
    });
    Self { inner }
  }

  pub async fn run(&self, context: SenderContextHandle, target: ExtendedPid, envelope: MessageEnvelope) {
    self.inner.run((context, target.to_core(), envelope)).await
  }

  pub fn from_core(inner: CoreSenderMiddlewareChain<SenderInvocation>) -> Self {
    Self { inner }
  }

  pub fn into_core(self) -> CoreSenderMiddlewareChain<SenderInvocation> {
    self.inner
  }

  pub fn as_core(&self) -> &CoreSenderMiddlewareChain<SenderInvocation> {
    &self.inner
  }
}

static_assertions::assert_impl_all!(SenderMiddlewareChain: Send, Sync);
