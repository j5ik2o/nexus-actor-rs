use std::env;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{MessagePart, SpawnerPart};
use crate::actor::core::props::Props;
use crate::actor::message::AutoReceiveMessage;
use crate::actor::message::Message;
use nexus_utils_std_rs::concurrent::AsyncBarrier;
use tracing_subscriber::EnvFilter;

#[tokio::test]
async fn example_root_context_spawn_named() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let b = AsyncBarrier::new(2);

  let system = ActorSystem::new().await.unwrap();
  let cloned_b = b.clone();

  let props = Props::from_async_actor_receiver(move |ctx| {
    let cloned_b = cloned_b.clone();
    async move {
      let msg = ctx.get_message_handle_opt().await.unwrap();
      if let Some(AutoReceiveMessage::PostStart) = msg.as_any().downcast_ref::<AutoReceiveMessage>() {
        tracing::debug!("Hello World!");
        cloned_b.wait().await;
      }
      Ok(())
    }
  })
  .await;

  if let Err(err) = system.get_root_context().await.spawn_named(props, "my-actor").await {
    panic!("Failed to spawn actor: {:?}", err);
  }

  b.wait().await;
}
