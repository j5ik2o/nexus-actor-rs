use crate::actor::actor::actor::Actor;
use crate::actor::actor::actor_error::ActorError;
use crate::actor::actor::actor_inner_error::ActorInnerError;
use crate::actor::context::ContextHandle;
use crate::actor::context::{BasePart, InfoPart, MessagePart, StopperPart};
use crate::actor::message::Message;
use async_trait::async_trait;
use futures::future::BoxFuture;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::sync::Arc;
use tokio::sync::Mutex;

type BehaviorFn<M> =
  Arc<dyn Fn(&mut TypedActorContext<M>) -> BoxFuture<'static, Result<Behavior<M>, ActorError>> + Send + Sync>;

#[derive(Clone)]
pub enum Behavior<M: Message> {
  Same,
  Stopped,
  Ignore,
  Unhandled,
  Func(BehaviorFn<M>),
}

impl<M: Message> Debug for Behavior<M> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Behavior")
  }
}

impl<M: Message + Clone> Behavior<M> {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(&mut TypedActorContext<M>) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<Behavior<M>, ActorError>> + Send + 'static, {
    Behavior::Func(Arc::new(move |ctx| Box::pin(f(ctx))))
  }

  pub async fn receive(&self, ctx: &mut TypedActorContext<M>) -> Result<Behavior<M>, ActorError> {
    match self {
      Behavior::Func(f) => f(ctx).await,
      Behavior::Same => Ok(self.clone()),
      Behavior::Stopped => Ok(self.clone()),
      Behavior::Ignore => Ok(self.clone()),
      Behavior::Unhandled => Ok(self.clone()),
    }
  }
}

#[derive(Debug, Clone)]
pub struct TypedActorContext<M: Message> {
  context_handle: ContextHandle,
  _phantom: std::marker::PhantomData<M>,
}

impl<M: Message + Clone> TypedActorContext<M> {
  pub fn new(context_handle: ContextHandle) -> Self {
    Self {
      context_handle,
      _phantom: std::marker::PhantomData,
    }
  }

  pub fn underlying(&self) -> &ContextHandle {
    &self.context_handle
  }

  pub fn underlying_mut(&mut self) -> &mut ContextHandle {
    &mut self.context_handle
  }

  pub async fn get_message_opt(&self) -> Option<M> {
    self.context_handle.get_message_handle().await.to_typed::<M>()
  }

  pub async fn get_message(&self) -> M {
    self.get_message_opt().await.unwrap()
  }
}

#[async_trait]
pub trait BehaviorActor: Debug {
  type Message: Message + Clone;

  fn create_initial_behavior() -> Result<Behavior<Self::Message>, ActorError>;
}

#[derive(Debug)]
struct TypedWrapper<A: BehaviorActor> {
  behavior: Arc<Mutex<Option<Behavior<A::Message>>>>,
}

impl<A: BehaviorActor> TypedWrapper<A> {
  fn new() -> Self {
    match A::create_initial_behavior() {
      Ok(behavior) => Self {
        behavior: Arc::new(Mutex::new(Some(behavior))),
      },
      Err(err) => panic!("Failed to create initial behavior: {:?}", err),
    }
  }
}

#[async_trait]
impl<A: BehaviorActor + 'static> Actor for TypedWrapper<A> {
  async fn receive(&mut self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
    let mut behavior_guard = self.behavior.lock().await;
    if let Some(current_behavior) = behavior_guard.take() {
      let mut actor_context = TypedActorContext::new(context_handle.clone());
      match current_behavior.receive(&mut actor_context).await {
        Ok(Behavior::Same) => {
          *behavior_guard = Some(current_behavior);
          Ok(())
        }
        Ok(Behavior::Stopped) => {
          *behavior_guard = Some(current_behavior);
          let self_pid = context_handle.get_self().await;
          context_handle.stop(&self_pid).await;
          Ok(())
        }
        Ok(Behavior::Ignore) => {
          *behavior_guard = Some(current_behavior.receive(&mut actor_context).await.unwrap());
          Ok(())
        }
        Ok(Behavior::Unhandled) => {
          context_handle.stash().await;
          *behavior_guard = Some(current_behavior);
          Ok(())
        }
        Ok(new_behavior) => {
          let result = context_handle.un_stash_all().await;
          if result.is_err() {
            return Err(result.unwrap_err());
          }
          *behavior_guard = Some(new_behavior);
          Ok(())
        }
        Err(err) => {
          *behavior_guard = Some(current_behavior);
          Err(err)
        }
      }
    } else {
      Err(ActorError::BehaviorNotInitialized(ActorInnerError::new(
        "Behavior not initialized".to_string(),
      )))
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::actor::actor::actor_error::ActorError;
  use crate::actor::actor::behavior_actor::{Behavior, BehaviorActor, TypedWrapper};
  use crate::actor::actor::props::Props;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{SenderPart, SpawnerPart};
  use crate::actor::message::Message;
  use crate::actor::message::MessageHandle;
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
    fn informal_behavior(greeting_count: usize) -> Result<Behavior<AppMessage>, ActorError> {
      Ok(Behavior::new(move |ctx| {
        let ctx = ctx.clone();
        async move {
          match ctx.get_message().await {
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
              Ok(Behavior::Same)
            }
          }
        }
      }))
    }

    fn formal_behavior(greeting_count: usize) -> Result<Behavior<AppMessage>, ActorError> {
      Ok(Behavior::new(move |ctx| {
        let ctx = ctx.clone();
        async move {
          match ctx.get_message().await {
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
              Ok(Behavior::Same)
            }
          }
        }
      }))
    }
  }

  impl BehaviorActor for StateSwitchingActor {
    type Message = AppMessage;

    fn create_initial_behavior() -> Result<Behavior<Self::Message>, ActorError> {
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
