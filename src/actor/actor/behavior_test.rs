#[cfg(test)]
mod tests {
  use crate::actor::actor::actor::Actor;
  use crate::actor::actor::actor_error::ActorError;
  use crate::actor::actor::behavior::Behavior;
  use crate::actor::actor::props::Props;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::context_handle::ContextHandle;
  use crate::actor::context::{BasePart, MessagePart, SenderPart, SpawnerPart};
  use crate::actor::message::message::Message;
  use crate::actor::message::message_handle::MessageHandle;
  use crate::actor::message::response::ResponseHandle;
  use async_trait::async_trait;
  use std::any::Any;
  use std::env;
  use std::sync::Arc;
  use std::time::Duration;
  use tokio::sync::Mutex;
  use tracing_subscriber::EnvFilter;

  #[derive(Debug, Clone, PartialEq, Eq)]
  struct BehaviorMessage;

  impl Message for BehaviorMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      let other_msg = other.as_any().downcast_ref::<BehaviorMessage>().unwrap();
      self == other_msg
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  struct EchoRequest;
  impl Message for EchoRequest {
    fn eq_message(&self, other: &dyn Message) -> bool {
      let other_msg = other.as_any().downcast_ref::<EchoRequest>().unwrap();
      self == other_msg
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[derive(Debug, PartialEq, Eq)]
  struct EchoResponse;

  impl Message for EchoResponse {
    fn eq_message(&self, other: &dyn Message) -> bool {
      let other_msg = other.as_any().downcast_ref::<EchoResponse>().unwrap();
      self == other_msg
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[derive(Debug, Clone)]
  struct EchoSetBehaviorActor {
    behavior: Arc<Mutex<Behavior>>,
  }

  impl EchoSetBehaviorActor {
    async fn new() -> Self {
      let actor = Self {
        behavior: Arc::new(Mutex::new(Behavior::new())),
      };
      let cloned_self = actor.clone();

      {
        tracing::debug!("EchoSetBehaviorActor::new:0");
        let mut behavior = actor.behavior.lock().await;
        tracing::debug!("EchoSetBehaviorActor::new:1");
        behavior
          .transition(move |ctx| {
            let cloned_self = cloned_self.clone();
            async move {
              tracing::debug!("EchoSetBehaviorActor::new:2");
              cloned_self.one(ctx).await
            }
          })
          .await;
        tracing::debug!("EchoSetBehaviorActor::new:3");
      }

      actor
    }

    async fn one(&self, ctx: ContextHandle) -> Result<(), ActorError> {
      tracing::debug!("one:0: {:?}", ctx.get_message_handle().await);
      if let Some(BehaviorMessage) = ctx.get_message_handle().await.to_typed::<BehaviorMessage>() {
        tracing::debug!("one:1: {:?}", ctx.get_message_handle().await);
        let cloned_self = self.clone();
        let mut behavior = self.behavior.lock().await;
        tracing::debug!("one:2: {:?}", ctx.get_message_handle().await);
        behavior
          .transition(move |ctx| {
            let cloned_self = cloned_self.clone();
            async move { cloned_self.other(ctx).await }
          })
          .await;
        tracing::debug!("one:3: {:?}", ctx.get_message_handle().await);
      }
      Ok(())
    }

    async fn other(&self, ctx: ContextHandle) -> Result<(), ActorError> {
      tracing::debug!("other: {:?}", ctx.get_message_handle().await);
      if let Some(EchoRequest) = ctx.get_message_handle().await.to_typed::<EchoRequest>() {
        ctx.respond(ResponseHandle::new(EchoResponse)).await;
      }
      Ok(())
    }
  }

  #[async_trait]
  impl Actor for EchoSetBehaviorActor {
    async fn receive(
      &mut self,
      context_handle: ContextHandle,
    ) -> Result<(), ActorError> {
      let mg = self.behavior.lock().await;
      mg.receive(context_handle).await
    }
  }
  // #[tokio::test]
  // async fn test_actor_can_set_behavior() {
  //     let _ = env::set_var("RUST_LOG", "debug");
  //     let _ = tracing_subscriber::fmt()
  //         .with_env_filter(EnvFilter::from_default_env())
  //         .try_init();
  //
  //     let system = ActorSystem::new().await;
  //     let mut root_context = system.get_root_context().await;
  //     let props = Props::from_actor_producer(|_| async { EchoSetBehaviorActor::new().await }).await;
  //     let pid = root_context.spawn(props).await;
  //
  //     root_context.send(pid.clone(), MessageHandle::new(BehaviorMessage)).await;
  //
  //     let future= root_context.request_future(pid, MessageHandle::new(EchoRequest), Duration::from_secs(3)).await;
  //
  //     let result = future.result().await;
  //
  //     println!("result: {:?}", result);
  //     assert!(result.is_ok());
  // }
}
