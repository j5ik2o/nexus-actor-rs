#[cfg(test)]
mod tests {
  use std::any::Any;
  use std::env;
  use std::time::Duration;

  use tokio::time::sleep;
  use tracing_subscriber::EnvFilter;

  use crate::actor::actor::props::Props;
  use crate::actor::actor::actor_receive_func::ActorReceiveFunc;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{BasePart, MessagePart, SenderPart, SpawnerPart, StopperPart};
  use crate::actor::message::message::Message;
  use crate::actor::message::message_handle::{MessageHandle};
  use crate::actor::message::message_or_envelope::MessageEnvelope;
  use crate::actor::message::response::{Response, ResponseHandle};
  use crate::actor::message::system_message::SystemMessage;
  use crate::actor::util::async_barrier::AsyncBarrier;

  #[tokio::test]
  async fn example() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let system = ActorSystem::new().await;
    let mut root_context = system.get_root_context().await;

    let props = Props::from_actor_receive_func(ActorReceiveFunc::new(move |ctx| async move {
      tracing::debug!("msg = {:?}", ctx.get_message().await.unwrap());
      Ok(())
    }))
        .await;

    let pid = root_context.spawn(props).await;
    root_context
        .send(pid.clone(), MessageHandle::new("Hello World".to_string()))
        .await;
    sleep(Duration::from_secs(1)).await;

    root_context.stop_future(&pid).await.result().await.unwrap();
  }

  #[derive(Debug)]
  struct Request(pub String);

  impl Message for Request {
    fn eq_message(&self, other: &dyn Message) -> bool {
      self.0 == other.as_any().downcast_ref::<Request>().unwrap().0
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }
  #[derive(Debug)]
  struct Reply(pub String);

  impl Message for Reply {
    fn eq_message(&self, other: &dyn Message) -> bool {
      self.0 == other.as_any().downcast_ref::<Reply>().unwrap().0
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  impl Response for Reply {
    fn eq_response(&self, other: &dyn Response) -> bool {
      self.0 == other.as_any().downcast_ref::<Reply>().unwrap().0
    }
  }

  #[tokio::test]
  async fn example_synchronous() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let b = AsyncBarrier::new(2);
    let cloned_b = b.clone();

    let system = ActorSystem::new().await;
    let mut root_context = system.get_root_context().await;

    let callee_props = Props::from_actor_receive_func(ActorReceiveFunc::new(move |ctx| async move {
      let msg = ctx.get_message().await.unwrap();
      tracing::debug!("callee msg = {:?}", msg);
      if let Some(msg) = msg.as_any().downcast_ref::<MessageEnvelope>() {
        tracing::debug!("{:?}", msg);
        ctx.respond(ResponseHandle::new(Reply("PONG".to_string()))).await
      }
      Ok(())
    }))
        .await;
    let callee_pid = root_context.spawn(callee_props).await;
    let cloned_callee_pid = callee_pid.clone();

    let caller_props = Props::from_actor_receive_func(ActorReceiveFunc::new(move |mut ctx| {
      let cloned_b = cloned_b.clone();
      let cloned_callee_pid = cloned_callee_pid.clone();
      async move {
        let msg = ctx.get_message().await.unwrap();
        tracing::debug!("caller msg = {:?}", msg);
        if let Some(msg) = msg.as_any().downcast_ref::<SystemMessage>() {
          if let SystemMessage::Started(_) = msg {
            ctx
                .request(cloned_callee_pid, MessageHandle::new(Request("PING".to_string())))
                .await;
          }
        }
        if let Some(msg) = msg.as_any().downcast_ref::<Reply>() {
          tracing::debug!("{:?}", msg);
          cloned_b.wait().await;
        }
        Ok(())
      }
    }))
        .await;
    let caller_pid = root_context.spawn(caller_props).await;

    b.wait().await;
    root_context.stop_future(&callee_pid).await.result().await.unwrap();
    root_context.stop_future(&caller_pid).await.result().await.unwrap();
  }
}