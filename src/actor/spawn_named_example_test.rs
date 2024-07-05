#![cfg(test)]
mod tests {
  use std::env;

  use tracing_subscriber::EnvFilter;

  use crate::actor::actor::props::Props;
  use crate::actor::actor::receive_func::ReceiveFunc;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{MessagePart, SpawnerPart};
  use crate::actor::message::message_handle::Message;
  use crate::actor::message::system_message::SystemMessage;

  use crate::actor::util::async_barrier::AsyncBarrier;

  #[tokio::test]
  async fn example_root_context_spawn_named() {
    let _ = env::set_var("RUST_LOG", "debug");
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
    let b = AsyncBarrier::new(2);

    let system = ActorSystem::new().await;
    let cloned_b = b.clone();

    let props = Props::from_receive_func(ReceiveFunc::new(move |ctx| {
      let cloned_b = cloned_b.clone();
      async move {
        let msg = ctx.get_message().await.unwrap();
        if let Some(SystemMessage::Started(_)) = msg.as_any().downcast_ref::<SystemMessage>() {
          println!("Hello World!");
          cloned_b.wait().await;
        }
        Ok(())
      }
    }))
        .await;

    let result = system.get_root_context().await.spawn_named(props, "my-actor").await;
    if let Err(err) = result {
      panic!("Failed to spawn actor: {:?}", err);
    }

    b.wait().await;
  }
}