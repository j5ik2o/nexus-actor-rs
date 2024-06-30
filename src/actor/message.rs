use std::any::Any;
use std::fmt::{Debug, Display, Formatter};
use std::future::Future;
use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::Mutex;

use crate::actor::actor::{ActorError, ActorHandle};
use crate::actor::context::{ContextHandle, ReceiverContextHandle, SenderContextHandle};
use crate::actor::message_envelope::MessageEnvelope;
use crate::actor::actor::pid::ExtendedPid;
use crate::util::element::Element;
use crate::util::queue::priority_queue::{PriorityMessage, DEFAULT_PRIORITY};

pub trait Response: Message + Debug + Send + Sync + 'static {}

#[derive(Debug, Clone)]
pub struct ResponseHandle(Arc<dyn Response>);

impl ResponseHandle {
  pub fn new(response: Arc<dyn Response>) -> Self {
    ResponseHandle(response)
  }
}

impl Message for ResponseHandle {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self.0.as_any()
  }
}

impl Response for ResponseHandle {}

pub trait Message: Debug + Send + Sync + 'static {
  fn get_priority(&self) -> i8 {
    DEFAULT_PRIORITY
  }
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static);
}

impl<T: prost::Message + 'static> Message for T {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

#[derive(Debug, Clone)]
pub struct MessageHandle(Arc<dyn Message>);

impl MessageHandle {
  pub fn new_arc(msg: Arc<dyn Message>) -> Self {
    if msg.as_any().downcast_ref::<MessageHandle>().is_some() {
      panic!("MessageHandle can't be used as a message, {:?}", msg);
    }
    MessageHandle(msg)
  }

  pub fn new(msg: impl Message + Send + Sync + 'static) -> Self {
    MessageHandle(Arc::new(msg))
  }
}

impl Element for MessageHandle {}

impl PriorityMessage for MessageHandle {
  fn get_priority(&self) -> Option<i8> {
    Some(self.0.get_priority())
  }
}

impl Message for MessageHandle {
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self.0.as_any()
  }
}

impl PartialEq for MessageHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for MessageHandle {}

impl std::hash::Hash for MessageHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Message).hash(state);
  }
}

impl Display for MessageHandle {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.to_string())
  }
}

// ---

#[derive(Debug, Clone)]
pub struct MessageHandles(Arc<Mutex<Vec<MessageHandle>>>);

impl MessageHandles {
  pub fn new(msgs: Vec<MessageHandle>) -> Self {
    Self(Arc::new(Mutex::new(msgs)))
  }

  pub async fn push(&mut self, msg: MessageHandle) {
    self.0.lock().await.push(msg);
  }

  pub async fn pop(&mut self) -> Option<MessageHandle> {
    self.0.lock().await.pop()
  }

  pub async fn len(&self) -> usize {
    self.0.lock().await.len()
  }

  pub async fn is_empty(&self) -> bool {
    self.0.lock().await.is_empty()
  }

  pub async fn clear(&mut self) {
    self.0.lock().await.clear();
  }

  pub async fn to_values(&self) -> Vec<MessageHandle> {
    self.0.lock().await.clone()
  }
}

// --- Producer

#[derive(Clone)]
pub struct ProducerFunc(Arc<dyn Fn(ContextHandle) -> BoxFuture<'static, ActorHandle> + Send + Sync>);

impl Debug for ProducerFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Producer")
  }
}

impl PartialEq for ProducerFunc {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ProducerFunc {}

impl std::hash::Hash for ProducerFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle) -> BoxFuture<'static, ActorHandle>).hash(state);
  }
}

impl ProducerFunc {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ActorHandle> + Send + 'static, {
    Self(Arc::new(move |ch| Box::pin(f(ch)) as BoxFuture<'static, ActorHandle>))
  }

  pub async fn run(&self, c: ContextHandle) -> ActorHandle {
    (self.0)(c).await
  }
}

#[derive(Clone)]
pub struct ReceiveFunc(Arc<dyn Fn(ContextHandle) -> BoxFuture<'static, Result<(), ActorError>> + Send + Sync>);

impl Debug for ReceiveFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ReceiveFunc")
  }
}

impl PartialEq for ReceiveFunc {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ReceiveFunc {}

impl std::hash::Hash for ReceiveFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle) -> BoxFuture<'static, Result<(), ActorError>>).hash(state);
  }
}

impl ReceiveFunc {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(ContextHandle) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), ActorError>> + Send + 'static, {
    ReceiveFunc(Arc::new(move |ch| {
      Box::pin(f(ch)) as BoxFuture<'static, Result<(), ActorError>>
    }))
  }

  pub async fn run(&self, context: ContextHandle) -> Result<(), ActorError> {
    (self.0)(context).await
  }
}

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
