#[cfg(test)]
pub mod tests {
  use async_trait::async_trait;
  use std::any::Any;
  use std::time::Duration;

  use crate::actor::actor::Actor;
  use crate::actor::actor::ActorError;
  use crate::actor::actor::Props;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::ContextHandle;
  use crate::actor::context::{BasePart, MessagePart, SenderPart, SpawnerPart};
  use crate::actor::message::Message;
  use crate::actor::message::MessageHandle;
  use crate::actor::message::ResponseHandle;

  #[derive(Debug, Clone)]
  pub struct DummyMessage;
  impl Message for DummyMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other.as_any().is::<DummyMessage>()
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[derive(Debug, Clone)]
  pub struct BlackHoleActor;

  #[async_trait]
  impl Actor for BlackHoleActor {
    async fn receive(&mut self, _: ContextHandle) -> Result<(), ActorError> {
      Ok(())
    }
  }

  #[derive(Debug, Clone)]
  pub struct EchoRequest;
  #[derive(Debug, Clone)]
  pub struct EchoResponse;

  impl Message for EchoRequest {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other.as_any().is::<EchoRequest>()
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  impl Message for EchoResponse {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other.as_any().is::<EchoResponse>()
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[derive(Debug, Clone)]
  pub struct EchoActor;

  #[async_trait]
  impl Actor for EchoActor {
    async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
      if let Some(_) = context_handle.get_message_handle().await.to_typed::<EchoRequest>() {
        context_handle.respond(ResponseHandle::new(EchoResponse)).await;
      }
      Ok(())
    }
  }

  #[tokio::test]
  async fn test_actor_can_reply_to_message() {
    let system = ActorSystem::new().await;
    let mut root_context = system.get_root_context().await;
    let pid = root_context
      .spawn(Props::from_actor_producer(|_| async { EchoActor }).await)
      .await;
    let result = root_context
      .request_future(pid, MessageHandle::new(EchoRequest), Duration::from_secs(1))
      .await
      .result()
      .await;
    assert!(result.is_ok());
  }
}
