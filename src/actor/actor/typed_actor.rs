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

type BehaviorFn<M> = Arc<dyn Fn(M, &mut TypedActorContext<M>) -> BoxFuture<'static, Behavior<M>> + Send + Sync>;

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
    F: Fn(M, &mut TypedActorContext<M>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Behavior<M>> + Send + 'static, {
    Behavior {
      f: Arc::new(move |msg, ctx| Box::pin(f(msg, ctx))),
    }
  }

  pub async fn receive(&self, msg: M, ctx: &mut TypedActorContext<M>) -> Behavior<M> {
    (self.f)(msg, ctx).await
  }
}

pub struct TypedActorContext<M: Message> {
  context_handle: ContextHandle,
  _phantom: std::marker::PhantomData<M>,
}

impl<M: Message> TypedActorContext<M> {
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
struct TypedWrapper<A: BehaviorActor> {
  behavior: Arc<Mutex<Option<Behavior<A::Message>>>>,
}

impl<A: BehaviorActor> TypedWrapper<A> {
  fn new() -> Self {
    Self {
      behavior: Arc::new(Mutex::new(Some(A::create_initial_behavior()))),
    }
  }
}

#[async_trait]
impl<A: BehaviorActor + 'static> Actor for TypedWrapper<A> {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let message_handle = context_handle.get_message_handle().await;
    let msg = message_handle.to_typed::<A::Message>().unwrap();

    let mut behavior_guard = self.behavior.lock().await;
    if let Some(current_behavior) = behavior_guard.take() {
      let mut actor_context = TypedActorContext::new(context_handle);
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

#[cfg(test)]
mod tests {
  use crate::actor::actor::props::Props;
  use crate::actor::actor::typed_actor::{Behavior, BehaviorActor, TypedWrapper};
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{SenderPart, SpawnerPart};
  use crate::actor::message::message::Message;
  use crate::actor::message::message_handle::MessageHandle;
  use std::any::Any;
  use std::time::Duration;
  use tokio::time::sleep;

  #[derive(Debug, Clone, PartialEq, Eq)]
  enum AppMessage {
    Greet(String),
    SwitchToFormal,
    SwitchToInformal,
  }

  impl Message for AppMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      match other.as_any().downcast_ref::<AppMessage>() {
        Some(other) => self == other,
        None => false,
      }
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[derive(Debug)]
  struct StateSwitchingActor;

  impl StateSwitchingActor {
    fn informal_behavior(greeting_count: usize) -> Behavior<AppMessage> {
      Behavior::new(move |msg, _ctx| async move {
        match msg {
          AppMessage::Greet(name) => {
            let new_count = greeting_count + 1;
            println!("Hey, {}! What's up? (Greetings: {})", name, new_count);
            Self::informal_behavior(new_count)
          }
          AppMessage::SwitchToFormal => {
            println!("Switching to formal behavior.");
            Self::formal_behavior(greeting_count)
          }
          _ => {
            println!("Informal: I don't understand that message.");
            Self::informal_behavior(greeting_count)
          }
        }
      })
    }

    fn formal_behavior(greeting_count: usize) -> Behavior<AppMessage> {
      Behavior::new(move |msg, _ctx| async move {
        match msg {
          AppMessage::Greet(name) => {
            let new_count = greeting_count + 1;
            println!("Good day, {}. How may I assist you? (Greetings: {})", name, new_count);
            Self::formal_behavior(new_count)
          }
          AppMessage::SwitchToInformal => {
            println!("Switching to informal behavior.");
            Self::informal_behavior(greeting_count)
          }
          _ => {
            println!("Formal: I do not understand that message.");
            Self::formal_behavior(greeting_count)
          }
        }
      })
    }
  }

  impl BehaviorActor for StateSwitchingActor {
    type Message = AppMessage;

    fn create_initial_behavior() -> Behavior<Self::Message> {
      Self::informal_behavior(0)
    }
  }

  #[tokio::test]
  async fn test() {
    let system = ActorSystem::new().await;
    let mut root_context = system.get_root_context().await;

    let pid = root_context
      .spawn(Props::from_actor_producer(|_| async { TypedWrapper::<StateSwitchingActor>::new() }).await)
      .await;

    root_context
      .send(pid.clone(), MessageHandle::new(AppMessage::Greet("Alice".to_string())))
      .await;
    root_context
      .send(pid.clone(), MessageHandle::new(AppMessage::SwitchToFormal))
      .await;
    root_context
      .send(pid.clone(), MessageHandle::new(AppMessage::Greet("Bob".to_string())))
      .await;
    root_context
      .send(pid.clone(), MessageHandle::new(AppMessage::SwitchToInformal))
      .await;
    root_context
      .send(
        pid.clone(),
        MessageHandle::new(AppMessage::Greet("Charlie".to_string())),
      )
      .await;

    sleep(Duration::from_secs(1)).await;
  }
}
