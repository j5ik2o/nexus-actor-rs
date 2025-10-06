use std::env;

use crate::actor::actor_system::ActorSystem;
use crate::actor::context::{MessagePart, SpawnerPart};
use crate::actor::core::props::Props;
use crate::actor::message::AutoReceiveMessage;
use crate::actor::message::Message;
use nexus_actor_core_rs::runtime::CoreSpawner as _;
use nexus_actor_core_rs::runtime::CoreTaskFuture;
use nexus_utils_std_rs::concurrent::AsyncBarrier;
use nexus_utils_std_rs::runtime::TokioCoreSpawner;
use tracing_subscriber::EnvFilter;

#[tokio::test]
async fn example_root_context_spawn() {
  env::set_var("RUST_LOG", "debug");
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();
  let b = AsyncBarrier::new(2);

  let system = ActorSystem::new().await.unwrap();
  let cloned_b = b.clone();

  let props = Props::from_async_actor_receiver(move |ctx| {
    let b = cloned_b.clone();
    async move {
      let msg = if let Some(handle) = ctx.try_get_message_handle_opt() {
        handle
      } else {
        ctx.get_message_handle_opt().await.expect("message not found")
      };
      tracing::debug!("msg = {:?}", msg);
      if let Some(AutoReceiveMessage::PreStart) = msg.as_any().downcast_ref::<AutoReceiveMessage>() {
        tracing::debug!("Hello World!");
        let task: CoreTaskFuture = Box::pin(async move {
          b.wait().await;
        });
        TokioCoreSpawner::current()
          .spawn(task)
          .expect("spawn barrier wait")
          .detach();
      }
      Ok(())
    }
  })
  .await;

  let pid = system.get_root_context().await.spawn(props).await;
  tracing::debug!("pid = {:?}", pid);

  b.wait().await;
}
