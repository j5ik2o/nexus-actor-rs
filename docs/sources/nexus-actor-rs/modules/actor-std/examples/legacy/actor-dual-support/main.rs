use async_trait::async_trait;
use nexus_actor_std_rs::actor::actor_system::ActorSystem;
use nexus_actor_std_rs::actor::context::{ContextHandle, MessagePart, SenderPart, SpawnerPart, StopperPart};
use nexus_actor_std_rs::actor::core::{Actor, ActorError, Props};
use nexus_actor_std_rs::actor::message::{Message, MessageHandle};
use nexus_message_derive_rs::Message as MessageDerive;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, MessageDerive)]
struct Greeting {
  name: String,
}

#[derive(Debug)]
struct FriendlyGreeter {
  id: String,
}

#[async_trait]
impl Actor for FriendlyGreeter {
  async fn receive(&mut self, ctx: ContextHandle) -> Result<(), ActorError> {
    let msg = ctx.get_message_handle_opt().await.expect("message not found");
    if let Some(greeting) = msg.to_typed::<Greeting>() {
      println!("[Friendly {}] Hello, {}!", self.id, greeting.name);
    }
    Ok(())
  }
}

#[derive(Debug)]
struct ExcitedGreeter {
  id: String,
}

#[async_trait]
impl Actor for ExcitedGreeter {
  async fn receive(&mut self, mut ctx: ContextHandle) -> Result<(), ActorError> {
    let msg = ctx.get_message_handle_opt().await.expect("message not found");
    if let Some(greeting) = msg.to_typed::<Greeting>() {
      println!("[Excited {}] WOW! Great to see you, {}!!!", self.id, greeting.name);
      if let Some(sender) = ctx.get_sender().await {
        ctx
          .send(
            sender,
            MessageHandle::new(Greeting {
              name: format!("{}'s friend", greeting.name),
            }),
          )
          .await;
      }
    }
    Ok(())
  }
}

#[tokio::main]
async fn main() {
  println!("=== Dual Actor Support Example (Actor トレイト統一) ===\n");

  let system = ActorSystem::new().await.expect("actor system");
  let mut root = system.get_root_context().await;

  let friendly_props = Props::from_async_actor_producer(|_| async { FriendlyGreeter { id: "F1".into() } }).await;
  let friendly_pid = root.spawn(friendly_props).await;

  let excited_props = Props::from_async_actor_producer(|_| async { ExcitedGreeter { id: "E1".into() } }).await;
  let excited_pid = root.spawn(excited_props).await;

  println!("Sending greetings...\n");
  root
    .send(
      friendly_pid.clone(),
      MessageHandle::new(Greeting {
        name: "Alice".to_string(),
      }),
    )
    .await;

  root
    .send(
      excited_pid.clone(),
      MessageHandle::new(Greeting {
        name: "Bob".to_string(),
      }),
    )
    .await;

  tokio::time::sleep(Duration::from_millis(200)).await;

  println!("\nForward interactions...");
  root
    .send(
      friendly_pid.clone(),
      MessageHandle::new(Greeting {
        name: "Charlie".to_string(),
      }),
    )
    .await;

  tokio::time::sleep(Duration::from_millis(200)).await;

  root.stop(&friendly_pid).await;
  root.stop(&excited_pid).await;

  tokio::time::sleep(Duration::from_millis(100)).await;
  println!("\n=== Example completed! ===");
}
