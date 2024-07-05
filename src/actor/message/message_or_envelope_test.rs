#[cfg(test)]
mod test {
  use std::any::Any;
  use std::env;
  use std::time::Duration;

  use tracing_subscriber::EnvFilter;

  use crate::actor::actor::props::Props;
  use crate::actor::actor::receive_func::ReceiveFunc;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{BasePart, MessagePart, SenderPart, SpawnerPart};
  use crate::actor::message::message_handle::{Message, MessageHandle};
  use crate::actor::message::message_or_envelope::{MessageEnvelope, ReadonlyMessageHeaders};
  use crate::actor::message::response::{Response, ResponseHandle};

  #[derive(Debug)]
  pub struct Length(pub usize);

  impl Message for Length {
    fn eq_message(&self, other: &dyn Message) -> bool {
      self.0 == other.as_any().downcast_ref::<Length>().unwrap().0
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }
  }

  impl Response for Length {
    fn eq_response(&self, other: &dyn Response) -> bool {
      self.0 == other.as_any().downcast_ref::<Length>().unwrap().0
    }
  }

  #[tokio::test]
  async fn test_normal_message_gives_empty_message_headers() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let system = ActorSystem::new().await;

    let props = Props::from_receive_func(ReceiveFunc::new(move |ctx| async move {
      let msg = ctx.get_message().await.unwrap();
      tracing::debug!("msg = {:?}", msg);
      if let Some(msg) = msg.as_any().downcast_ref::<MessageEnvelope>() {
        tracing::debug!("msg = {:?}", msg);
        let l = ctx.get_message_header().await.map(|v| v.keys().len()).unwrap_or(0);
        ctx.respond(ResponseHandle::new(Length(l))).await
      }
      Ok(())
    }))
        .await;

    let mut root_context = system.get_root_context().await;
    let pid = root_context.spawn(props).await;

    let d = Duration::from_secs(1);

    let f = root_context
        .request_future(pid, MessageHandle::new("Hello".to_string()), &d)
        .await;

    let _ = f.result().await.unwrap();
  }
}