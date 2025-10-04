use async_trait::async_trait;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::ContextHandle;
use nexus_actor_std_rs::actor::context::{MessagePart, SenderPart, SpawnerPart};
use nexus_actor_std_rs::actor::core::Actor;
use nexus_actor_std_rs::actor::core::ActorError;
use nexus_actor_std_rs::actor::core::Props;
use nexus_actor_std_rs::actor::message::Message;
use nexus_actor_std_rs::actor::message::MessageHandle;
use nexus_actor_std_rs::Message;
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct Hello {
  who: String,
}

#[derive(Debug)]
struct HelloActor;

#[async_trait]
impl Actor for HelloActor {
  async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let message_handle = ctx.get_message_handle_opt().await.expect("message not found");
    let hello = message_handle.to_typed::<Hello>().unwrap();
    tracing::info!("Hello, {}!", hello.who);
    Ok(())
  }
}

#[tokio::main]
async fn main() {
  env::set_var("RUST_LOG", "actor_hello_world=info");
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();

  let system = ActorSystem::new().await.unwrap();
  let mut root_context = system.get_root_context().await;
  let actor_producer = |_| async { HelloActor };
  let pid = root_context
    .spawn(Props::from_async_actor_producer(actor_producer).await)
    .await;
  root_context
    .send(
      pid,
      MessageHandle::new(Hello {
        who: "world".to_string(),
      }),
    )
    .await;
  sleep(Duration::from_secs(1)).await;
}
