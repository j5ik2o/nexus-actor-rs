use async_trait::async_trait;
use nexus_acto_rs::actor::actor::actor::Actor;
use nexus_acto_rs::actor::actor::actor_error::ActorError;
use nexus_acto_rs::actor::actor::props::Props;
use nexus_acto_rs::actor::actor_system::ActorSystem;
use nexus_acto_rs::actor::context::context_handle::ContextHandle;
use nexus_acto_rs::actor::context::{MessagePart, SenderPart, SpawnerPart};
use nexus_acto_rs::actor::message::message::Message;
use nexus_acto_rs::actor::message::message_handle::MessageHandle;
use std::any::Any;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone)]
struct Hello {
  who: String,
}

impl Message for Hello {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.eq_message(self)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
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
