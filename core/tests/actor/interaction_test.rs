  use nexus_actor_core_rs::actor::core::Actor;
  use nexus_actor_core_rs::actor::core::ActorError;
  use nexus_actor_core_rs::actor::core::Props;
  use nexus_actor_core_rs::actor::actor_system::ActorSystem;
  use nexus_actor_core_rs::actor::context::ContextHandle;
  use nexus_actor_core_rs::actor::context::{BasePart, MessagePart, SenderPart, SpawnerPart};
  use nexus_actor_core_rs::actor::message::Message;
  use nexus_actor_core_rs::actor::message::MessageHandle;
  use nexus_actor_core_rs::actor::message::ResponseHandle;
  use async_trait::async_trait;
  use nexus_actor_message_derive_rs::Message;
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
      if let Some(_) = context_handle.get_message_handle().await.to_typed::<EchoRequest>() {
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

