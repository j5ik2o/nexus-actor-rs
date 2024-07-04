use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::ActorError;
use crate::actor::context::{ContextHandle, ReceiverContextHandle, SenderContextHandle};
use crate::util::element::Element;
use crate::util::queue::priority_queue::PriorityMessage;
use message_envelope::MessageEnvelope;

pub mod auto_receive_message;
pub mod failure;
pub mod ignore_dead_letter_logging;
pub mod message_envelope;
pub mod message_handle;
pub mod message_handles;
pub mod not_influence_receive_timeout;
pub mod receive_timeout;
pub mod response;
pub mod system_message;

// ReceiverFunc
#[derive(Clone)]
pub struct ReceiverFunc(
  Arc<dyn Fn(ReceiverContextHandle, MessageEnvelope) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync>,
);

impl Debug for ReceiverFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ReceiverFunc")
  }
}

impl PartialEq for ReceiverFunc {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ReceiverFunc {}

impl std::hash::Hash for ReceiverFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref()
      as *const dyn Fn(ReceiverContextHandle, MessageEnvelope) -> BoxFuture<'static, Result<(), ActorError>>)
      .hash(state);
  }
}

impl ReceiverFunc {
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

// SenderFunc
#[derive(Clone)]
pub struct SenderFunc(
  Arc<dyn Fn(SenderContextHandle, ExtendedPid, MessageEnvelope) -> BoxFuture<'static, ()> + Send + Sync>,
);

impl Debug for SenderFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SenderFunc")
  }
}

impl PartialEq for SenderFunc {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for SenderFunc {}

impl std::hash::Hash for SenderFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(SenderContextHandle, ExtendedPid, MessageEnvelope) -> BoxFuture<'static, ()>)
      .hash(state);
  }
}

impl SenderFunc {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(SenderContextHandle, ExtendedPid, MessageEnvelope) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ()> + Send + 'static, {
    Self(Arc::new(move |sch, ep, me| {
      Box::pin(f(sch, ep, me)) as BoxFuture<'static, ()>
    }))
  }

  pub async fn run(&self, context: SenderContextHandle, target: ExtendedPid, envelope: MessageEnvelope) {
    (self.0)(context, target, envelope).await;
  }
}

// ContextDecoratorFunc
#[derive(Clone)]
pub struct ContextDecoratorFunc(Arc<dyn Fn(ContextHandle) -> BoxFuture<'static, ContextHandle> + Send + Sync>);

impl Debug for ContextDecoratorFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContextDecoratorFunc")
  }
}

impl PartialEq for ContextDecoratorFunc {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ContextDecoratorFunc {}

impl std::hash::Hash for ContextDecoratorFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle) -> BoxFuture<'static, ContextHandle>).hash(state);
  }
}

impl ContextDecoratorFunc {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ContextHandle> + Send + 'static, {
    Self(Arc::new(move |ch| Box::pin(f(ch)) as BoxFuture<'static, ContextHandle>))
  }

  pub async fn run(&self, context: ContextHandle) -> ContextHandle {
    (self.0)(context).await
  }
}
