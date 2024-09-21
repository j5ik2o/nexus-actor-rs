use crate::actor::actor::{Actor, ActorError, ActorReceiver};
use crate::actor::context::{ContextHandle, InfoPart};
use crate::actor::util::stack::Stack;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ActorBehavior {
  stack: Arc<Mutex<Stack<ActorReceiver>>>,
}

impl ActorBehavior {
  pub fn new() -> Self {
    Self {
      stack: Arc::new(Mutex::new(Stack::new())),
    }
  }

  pub async fn reset(&mut self, receiver: ActorReceiver) {
    let mut mg = self.stack.lock().await;
    mg.clear();
    mg.push(receiver);
  }

  pub async fn become_stacked(&mut self, receiver: ActorReceiver) {
    let mut mg = self.stack.lock().await;
    mg.push(receiver);
  }

  pub async fn un_become_stacked(&mut self) {
    let mut mg = self.stack.lock().await;
    mg.pop();
  }

  pub async fn clear(&mut self) {
    let mut mg = self.stack.lock().await;
    mg.clear();
  }
}

#[async_trait]
impl Actor for ActorBehavior {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    if let Some(behavior) = {
      let mg = self.stack.lock().await;
      mg.peek()
    } {
      behavior.run(context_handle.clone()).await?;
    } else {
      tracing::error!("empty behavior called: pid = {}", context_handle.get_self().await);
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use crate::actor::actor::actor_behavior::ActorBehavior;
  use crate::actor::actor::{Actor, ActorError, ActorReceiver, Props};
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{BasePart, ContextHandle, MessagePart, SenderPart, SpawnerPart};
  use crate::actor::message::{Message, MessageHandle, ResponseHandle};
  use async_trait::async_trait;
  use nexus_actor_message_derive_rs::Message;
  use std::env;
  use std::time::Duration;
  use tracing_subscriber::EnvFilter;

  #[derive(Debug, Clone, PartialEq, Message)]
  struct BehaviorMessage;

  #[derive(Debug, Clone, PartialEq, Message)]
  struct EchoRequest;

  #[derive(Debug, Clone, PartialEq, Message)]
  struct EchoResponse;

  #[derive(Debug, Clone)]
  struct EchoSetBehaviorActor {
    behavior: ActorBehavior,
  }

  impl EchoSetBehaviorActor {
    async fn new() -> Self {
      let mut actor = Self {
        behavior: ActorBehavior::new(),
      };
      let cloned_self = actor.clone();
      actor.behavior.clear().await;
      actor
        .behavior
        .become_stacked(ActorReceiver::new(move |ctx| {
          let mut cloned_self = cloned_self.clone();
          async move { cloned_self.one(ctx).await }
        }))
        .await;
      actor
    }

    async fn one(&mut self, context: ContextHandle) -> Result<(), ActorError> {
      tracing::debug!("one: {:?}", context.get_message_handle().await);
      if context
        .get_message_handle()
        .await
        .to_typed::<BehaviorMessage>()
        .is_some()
      {
        let cloned_self = self.clone();
        self
          .behavior
          .become_stacked(ActorReceiver::new(move |ctx| {
            let mut cloned_self = cloned_self.clone();
            async move { cloned_self.other(ctx).await }
          }))
          .await;
      }
      Ok(())
    }

    async fn other(&mut self, context: ContextHandle) -> Result<(), ActorError> {
      tracing::debug!("other: {:?}", context.get_message_handle().await);
      if context.get_message_handle().await.to_typed::<EchoRequest>().is_some() {
        context.respond(ResponseHandle::new(EchoResponse)).await;
      }
      Ok(())
    }
  }

  #[async_trait]
  impl Actor for EchoSetBehaviorActor {
    async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
      tracing::debug!(
        "EchoSetBehaviorActor::receive: {:?}",
        context_handle.get_message_handle().await
      );
      self.behavior.receive(context_handle).await
    }
  }

  #[tokio::test]
  async fn test_actor_can_set_behavior() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await.unwrap();
    let mut root_context = system.get_root_context().await;
    let pid = root_context
      .spawn(Props::from_actor_producer(|_| async { EchoSetBehaviorActor::new().await }).await)
      .await;

    root_context
      .send(pid.clone(), MessageHandle::new(BehaviorMessage))
      .await;
    let response = root_context
      .request_future(pid, MessageHandle::new(EchoRequest), Duration::from_secs(1))
      .await
      .result()
      .await;
    assert!(response.is_ok());
  }
}
