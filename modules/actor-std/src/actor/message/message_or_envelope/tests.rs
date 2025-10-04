#[cfg(test)]
mod test {
  use std::env;
  use std::time::Duration;

  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{BasePart, MessagePart, SenderPart, SpawnerPart};
  use crate::actor::core::Props;
  use crate::actor::message::message_base::Message;
  use crate::actor::message::message_handle::MessageHandle;
  use crate::actor::message::response::ResponseHandle;
  use nexus_message_derive_rs::Message;
  use tracing_subscriber::EnvFilter;

  #[derive(Debug, Clone, PartialEq, Eq, Message)]
  pub struct Length(pub usize);

  #[tokio::test]
  async fn test_normal_message_gives_empty_message_headers() {
    env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
      .with_env_filter(EnvFilter::from_default_env())
      .try_init();

    let system = ActorSystem::new().await.unwrap();

    let props = Props::from_async_actor_receiver(move |ctx| async move {
      let msg = ctx.get_message_envelope_opt().await;
      if msg.is_some() {
        let l = ctx
          .get_message_header_handle()
          .await
          .map(|v| v.keys().len())
          .unwrap_or(0);
        ctx.respond(ResponseHandle::new(Length(l))).await
      }
      Ok(())
    })
    .await;

    let mut root_context = system.get_root_context().await;
    let pid = root_context.spawn(props).await;

    let d = Duration::from_secs(1);

    let f = root_context
      .request_future(pid, MessageHandle::new("Hello".to_string()), d)
      .await;

    let _ = f.result().await.unwrap();
  }
}
