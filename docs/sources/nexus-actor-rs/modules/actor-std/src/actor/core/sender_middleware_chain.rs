use std::fmt::{Debug, Formatter};
use std::future::Future;

use futures::future::BoxFuture;
use nexus_actor_core_rs::context::{CoreSenderInvocation, CoreSenderMiddlewareChain};
use nexus_actor_core_rs::CorePid;

use crate::actor::actor_system::with_actor_system;
use crate::actor::context::SenderContextHandle;
use crate::actor::core::pid::ExtendedPid;
use crate::actor::message::{MessageEnvelope, MessageHandle};

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

  pub fn to_core_invocation_chain(&self) -> CoreSenderMiddlewareChain<CoreSenderInvocation> {
    let inner = self.inner.clone();
    CoreSenderMiddlewareChain::new(move |invocation: CoreSenderInvocation| {
      let inner = inner.clone();
      Box::pin(async move {
        let (snapshot, target_core, envelope_core) = invocation.into_parts();
        let target_core_clone = target_core.clone();
        let message_envelope = MessageEnvelope::from_core(envelope_core.clone());

        if let Some(sender_ctx) = SenderContextHandle::from_core_snapshot(&snapshot).await {
          inner
            .run((sender_ctx, target_core_clone, message_envelope.clone()))
            .await;
        } else if let Some(system) = with_actor_system(snapshot.actor_system_id(), |sys| sys.clone()) {
          let fallback_target = ExtendedPid::from_core(target_core);
          fallback_target
            .send_user_message(system, MessageHandle::new(message_envelope))
            .await;
        }
      }) as BoxFuture<'static, ()>
    })
  }
}

static_assertions::assert_impl_all!(SenderMiddlewareChain: Send, Sync);
