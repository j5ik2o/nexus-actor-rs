#[cfg(test)]
pub mod tests {
  use crate::actor::actor::actor::Actor;
  use crate::actor::actor::actor_error::ActorError;
  use crate::actor::actor::actor_inner_error::ActorInnerError;
  use crate::actor::actor::pid::ExtendedPid;
  use crate::actor::actor::props::Props;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::context_handle::ContextHandle;
  use crate::actor::context::{BasePart, MessagePart, SenderPart, SpawnerPart, StopperPart};
  use crate::actor::future::FutureProcess;
  use crate::actor::interaction_test::tests::BlackHoleActor;
  use crate::actor::message::message::Message;
  use crate::actor::message::message_handle::MessageHandle;
  use crate::actor::message::response::ResponseHandle;
  use async_trait::async_trait;
  use std::any::Any;
  use std::env;
  use std::time::Duration;
  use tracing_subscriber::EnvFilter;

  #[derive(Debug, Clone)]
  struct CreateChildMessage;

  impl Message for CreateChildMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other.as_any().is::<CreateChildMessage>()
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }
  #[derive(Debug, Clone)]
  struct GetChildCountRequest;

  impl Message for GetChildCountRequest {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other.as_any().is::<GetChildCountRequest>()
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }
  #[derive(Debug, Clone)]
  struct GetChildCountResponse {
    child_count: usize,
  }

  impl Message for GetChildCountResponse {
    fn eq_message(&self, other: &dyn Message) -> bool {
      let other_msg = other.as_any().downcast_ref::<GetChildCountResponse>();
      match other_msg {
        Some(other_msg) => self.child_count == other_msg.child_count,
        None => false,
      }
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[derive(Debug)]
  struct CreateChildActor;

  #[async_trait]
  impl Actor for CreateChildActor {
    async fn receive(&mut self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
      let msg = context_handle.get_message_handle().await;
      if let Some(_) = msg.to_typed::<CreateChildMessage>() {
        context_handle
          .spawn(Props::from_actor_producer(|_| async { BlackHoleActor }).await)
          .await;
      } else if let Some(_) = msg.to_typed::<GetChildCountRequest>() {
        let reply = GetChildCountResponse {
          child_count: context_handle.get_children().await.len(),
        };
        context_handle.respond(ResponseHandle::new(reply)).await;
      } else {
        return Err(ActorError::ReceiveError(ActorInnerError::new("Unknown message")));
      }

      Ok(())
    }
  }

  #[tokio::test]
  async fn test_actor_can_create_children() {
    let system = ActorSystem::new().await;
    let mut root_context = system.get_root_context().await;
    let pid = root_context
      .spawn(Props::from_actor_producer(|_| async { CreateChildActor }).await)
      .await;
    let expected = 10;
    for _ in 0..expected {
      root_context
        .send(pid.clone(), MessageHandle::new(CreateChildMessage))
        .await;
    }
    let fut = root_context
      .request_future(
        pid.clone(),
        MessageHandle::new(GetChildCountRequest),
        std::time::Duration::from_secs(1),
      )
      .await;
    let response = fut.result().await.unwrap();
    let response = response.to_typed::<GetChildCountResponse>().unwrap();
    assert_eq!(response.child_count, expected);
  }

  #[derive(Debug, Clone)]
  struct GetChildCountMessage2 {
    reply_directly: ExtendedPid,
    reply_after_stop: ExtendedPid,
  }

  impl Message for GetChildCountMessage2 {
    fn eq_message(&self, other: &dyn Message) -> bool {
      let other_msg = other.as_any().downcast_ref::<GetChildCountMessage2>();
      match other_msg {
        Some(other_msg) => {
          self.reply_directly == other_msg.reply_directly && self.reply_after_stop == other_msg.reply_after_stop
        }
        None => false,
      }
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  #[derive(Debug, Clone)]
  struct CreateChildThenStopActor {
    reply_to: Option<ExtendedPid>,
  }

  #[async_trait]
  impl Actor for CreateChildThenStopActor {
    async fn receive(&mut self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
      let msg = context_handle.get_message_handle().await;
      tracing::debug!("CreateChildThenStopActor: {:?}", msg);
      if let Some(_) = msg.to_typed::<CreateChildMessage>() {
        context_handle
          .spawn(Props::from_actor_producer(|_| async { BlackHoleActor }).await)
          .await;
        Ok(())
      } else if let Some(msg) = msg.to_typed::<GetChildCountMessage2>() {
        context_handle
          .send(msg.reply_directly.clone(), MessageHandle::new(true))
          .await;
        self.reply_to = Some(msg.reply_after_stop.clone());
        Ok(())
      } else {
        Ok(())
      }
    }

    async fn post_stop(&self, mut context_handle: ContextHandle) -> Result<(), ActorError> {
      tracing::debug!(
        "post_stop: children.len = {:?}",
        context_handle.get_children().await.len()
      );
      let reply = GetChildCountResponse {
        child_count: context_handle.get_children().await.len(),
      };
      context_handle
        .send(self.reply_to.clone().unwrap(), MessageHandle::new(reply))
        .await;
      Ok(())
    }
  }

  #[tokio::test]
  async fn test_actor_can_stop_children() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await;
    let mut root_context = system.get_root_context().await;
    let a = root_context
      .spawn(Props::from_actor_producer(|_| async { CreateChildThenStopActor { reply_to: None } }).await)
      .await;

    let count = 10;
    for _ in 0..count {
      root_context
        .send(a.clone(), MessageHandle::new(CreateChildMessage))
        .await;
    }

    let future1 = FutureProcess::new(system.clone(), Duration::from_secs(5)).await;
    let future2 = FutureProcess::new(system.clone(), Duration::from_secs(5)).await;

    root_context
      .send(
        a.clone(),
        MessageHandle::new(GetChildCountMessage2 {
          reply_directly: future1.get_pid().await,
          reply_after_stop: future2.get_pid().await,
        }),
      )
      .await;

    let r1 = future1.result().await.unwrap();
    assert!(r1.to_typed::<bool>().unwrap());
    root_context.stop(&a).await;

    let r2 = future2.result().await.unwrap();
    assert_eq!(0, r2.to_typed::<GetChildCountResponse>().unwrap().child_count);
  }
}
