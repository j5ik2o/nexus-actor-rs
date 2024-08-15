use async_trait::async_trait;
use nexus_acto_message_derive_rs::Message;
use nexus_acto_rs::actor::actor::Actor;
use nexus_acto_rs::actor::actor::ActorError;
use nexus_acto_rs::actor::actor::Props;
use nexus_acto_rs::actor::actor_system::ActorSystem;
use nexus_acto_rs::actor::context::ContextHandle;
use nexus_acto_rs::actor::context::{MessagePart, SenderPart, SpawnerPart};
use nexus_acto_rs::actor::message::Message;
use nexus_acto_rs::actor::message::MessageHandle;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
struct Hello {
  who: String,
}

#[derive(Debug)]
struct HelloActor;

#[async_trait]
impl Actor for HelloActor {
  async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let message_handle = ctx.get_message_handle().await;
    let hello = message_handle.to_typed::<Hello>().unwrap();
    println!("Hello, {}!", hello.who);
    Ok(())
  }
}

#[tokio::main]
async fn main() {
  let system = ActorSystem::new().await;
  let mut root_context = system.get_root_context().await;
  let actor_producer = |_| async { HelloActor };
  let pid = root_context
    .spawn(Props::from_actor_producer(actor_producer).await)
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
