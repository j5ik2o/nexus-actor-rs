use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{BasePart, MessagePart};
use crate::actor::context::{ContextHandle, SenderPart, SpawnerPart};
use crate::actor::core::Actor;
use crate::actor::core::ActorError;
use crate::actor::core::Props;
use crate::actor::message::Message;
use crate::actor::message::MessageHandle;
use crate::actor::message::ResponseHandle;
use async_trait::async_trait;
use nexus_message_derive_rs::Message;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct DummyMessage;

#[derive(Debug, Clone)]
pub struct BlackHoleActor;

#[async_trait]
impl Actor for BlackHoleActor {
  async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct EchoRequest;
#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct EchoResponse;

#[derive(Debug, Clone)]
pub struct EchoActor;

#[async_trait]
impl Actor for EchoActor {
  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let message_handle = if let Some(handle) = context_handle.try_get_message_handle_opt() {
      handle
    } else {
      context_handle
        .get_message_handle_opt()
        .await
        .expect("message not found")
    };
    if message_handle.to_typed::<EchoRequest>().is_some() {
      context_handle.respond(ResponseHandle::new(EchoResponse)).await;
    }
    Ok(())
  }
}

#[tokio::test]
async fn test_actor_can_reply_to_message() {
  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;
  let pid = root_context
    .spawn(Props::from_async_actor_producer(|_| async { EchoActor }).await)
    .await;
  let result = root_context
    .request_future(pid, MessageHandle::new(EchoRequest), Duration::from_secs(1))
    .await
    .result()
    .await;
  assert!(result.is_ok());
}
