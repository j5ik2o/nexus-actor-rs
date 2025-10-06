use async_trait::async_trait;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{ContextHandle, MessagePart, SenderPart, SpawnerPart, StopperPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, Props};
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use nexus_message_derive_rs::Message as MessageDerive;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, MessageDerive)]
struct Hello {
  name: String,
}

#[derive(Debug)]
struct GreeterActor;

#[async_trait]
impl Actor for GreeterActor {
  async fn pre_start(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    println!("GreeterActor starting!");
    Ok(())
  }

  async fn receive(&mut self, context: ContextHandle) -> Result<(), ActorError> {
    if let Some(hello) = context
      .get_message_handle_opt()
      .await
      .expect("message not found")
      .to_typed::<Hello>()
    {
      println!("Hello, {}!", hello.name);
    }
    Ok(())
  }

  async fn post_stop(&mut self, _: ContextHandle) -> Result<(), ActorError> {
    println!("GreeterActor stopped!");
    Ok(())
  }
}

#[tokio::main]
async fn main() {
  let system = ActorSystem::new().await.expect("actor system");
  let mut root = system.get_root_context().await;

  let props = Props::from_async_actor_producer(|_| async { GreeterActor }).await;
  let pid = root.spawn(props).await;

  root
    .send(
      pid.clone(),
      MessageHandle::new(Hello {
        name: "World".to_string(),
      }),
    )
    .await;

  tokio::time::sleep(Duration::from_millis(100)).await;
  root.stop(&pid).await;
  tokio::time::sleep(Duration::from_millis(100)).await;

  println!("Example completed!");
}
