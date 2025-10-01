use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{BasePart, ContextHandle, MessagePart, SenderPart, SpawnerPart};
use crate::actor::core::actor_behavior::ActorBehavior;
use crate::actor::core::{Actor, ActorError, ActorReceiver, Props};
use crate::actor::message::{Message, MessageHandle, ResponseHandle};
use async_trait::async_trait;
use nexus_message_derive_rs::Message;
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
    tracing::debug!(
      "one: {:?}",
      context.get_message_handle_opt().await.expect("message not found")
    );
    if context
      .get_message_handle_opt()
      .await
      .expect("message not found")
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
    tracing::debug!(
      "other: {:?}",
      context.get_message_handle_opt().await.expect("message not found")
    );
    if context
      .get_message_handle_opt()
      .await
      .expect("message not found")
      .to_typed::<EchoRequest>()
      .is_some()
    {
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
      context_handle
        .get_message_handle_opt()
        .await
        .expect("message not found")
    );
    self.behavior.receive(context_handle).await
  }
}

#[tokio::test]
async fn test_actor_can_set_behavior() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;
  let pid = root_context
    .spawn(Props::from_async_actor_producer(|_| async { EchoSetBehaviorActor::new().await }).await)
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
