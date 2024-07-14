use crate::actor::actor::actor::Actor;
use crate::actor::actor::actor_error::ActorError;
use crate::actor::actor::actor_inner_error::ActorInnerError;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::context::MessagePart;
use crate::actor::message::message::Message;
use async_trait::async_trait;
use futures::future::BoxFuture;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;

type BehaviorFn<M> = Arc<dyn Fn(M, &mut ActorContext<M>) -> BoxFuture<'static, Behavior<M>> + Send + Sync>;

#[derive(Clone)]
pub struct Behavior<M: Message> {
  f: BehaviorFn<M>,
}

impl<M: Message> Debug for Behavior<M> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Behavior")
  }
}

impl<M: Message> Behavior<M> {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(M, &mut ActorContext<M>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Behavior<M>> + Send + 'static, {
    Behavior {
      f: Arc::new(move |msg, ctx| Box::pin(f(msg, ctx))),
    }
  }

  pub async fn receive(&self, msg: M, ctx: &mut ActorContext<M>) -> Behavior<M> {
    (self.f)(msg, ctx).await
  }
}

pub struct ActorContext<M: Message> {
  context_handle: ContextHandle,
  _phantom: std::marker::PhantomData<M>,
}

impl<M: Message> ActorContext<M> {
  pub fn new(context_handle: ContextHandle) -> Self {
    Self {
      context_handle,
      _phantom: std::marker::PhantomData,
    }
  }

  pub fn context_handle(&self) -> &ContextHandle {
    &self.context_handle
  }
}

#[async_trait]
pub trait BehaviorActor: Debug {
  type Message: Message + Clone;

  fn create_initial_behavior() -> Behavior<Self::Message>;
}

#[derive(Debug)]
struct ActorWrapper<A: BehaviorActor> {
  behavior: Arc<Mutex<Option<Behavior<A::Message>>>>,
}

impl<A: BehaviorActor> ActorWrapper<A> {
  fn new() -> Self {
    Self {
      behavior: Arc::new(Mutex::new(Some(A::create_initial_behavior()))),
    }
  }
}

#[async_trait]
impl<A: BehaviorActor + 'static> Actor for ActorWrapper<A> {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let message_handle = context_handle.get_message_handle().await;
    let msg = message_handle.to_typed::<A::Message>().unwrap();

    let mut behavior_guard = self.behavior.lock().await;
    if let Some(current_behavior) = behavior_guard.take() {
      let mut actor_context = ActorContext::new(context_handle);
      let new_behavior = current_behavior.receive(msg, &mut actor_context).await;
      *behavior_guard = Some(new_behavior);
    } else {
      return Err(ActorError::BehaviorNotInitialized(ActorInnerError::new(
        "Behavior not initialized".to_string(),
      )));
    }
    Ok(())
  }
}
