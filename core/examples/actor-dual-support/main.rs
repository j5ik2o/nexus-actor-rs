use async_trait::async_trait;
use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{BaseSpawnerExt, SenderPart, SpawnerPart, StopperPart};
use nexus_actor_core_rs::actor::core::{Actor, ActorError, Props};
use nexus_actor_core_rs::actor::core_types::{BaseActor, BaseActorError, BaseContext, Message};
use nexus_actor_core_rs::actor::message::MessageHandle;
use std::any::Any;
use std::fmt::Debug;

// Shared message type that both actor types can handle
#[derive(Debug, Clone)]
struct Greeting {
  name: String,
}

impl Message for Greeting {
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other_greeting) = other.as_any().downcast_ref::<Greeting>() {
      self.name == other_greeting.name
    } else {
      false
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    "Greeting".to_string()
  }
}

// Traditional Actor implementation
#[derive(Debug)]
struct TraditionalGreeter {
  id: String,
}

#[async_trait]
impl Actor for TraditionalGreeter {
  async fn receive(&mut self, context: nexus_actor_core_rs::actor::context::ContextHandle) -> Result<(), ActorError> {
    use nexus_actor_core_rs::actor::context::MessagePart;

    let msg = context.get_message_handle().await;
    if let Some(greeting) = msg.to_typed::<Greeting>() {
      println!("[Traditional {}] Hello, {}!", self.id, greeting.name);
    }
    Ok(())
  }
}

// New BaseActor implementation
#[derive(Debug)]
struct ModernGreeter {
  id: String,
}

#[async_trait]
impl BaseActor for ModernGreeter {
  async fn handle(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
    let msg = context.get_message().await;
    if let Some(greeting) = msg.to_typed::<Greeting>() {
      println!("[Modern {}] Greetings, {}!", self.id, greeting.name);
    }
    Ok(())
  }

  async fn pre_start(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
    println!("[Modern {}] Starting up!", self.id);
    Ok(())
  }

  async fn post_stop(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
    println!("[Modern {}] Shutting down!", self.id);
    Ok(())
  }
}

#[tokio::main]
async fn main() {
  println!("=== Dual Actor Support Example ===\n");

  // Create actor system
  let actor_system = ActorSystem::new().await.unwrap();
  let mut root_context = actor_system.get_root_context().await;

  println!("Spawning traditional actors...");

  // Spawn traditional actors
  let trad_props1 = Props::from_async_actor_receiver(|ctx| {
    let mut actor = TraditionalGreeter { id: "T1".to_string() };
    async move { actor.receive(ctx).await }
  })
  .await;
  let trad_pid1 = root_context.spawn(trad_props1).await;

  let trad_props2 = Props::from_async_actor_receiver(|ctx| {
    let mut actor = TraditionalGreeter { id: "T2".to_string() };
    async move { actor.receive(ctx).await }
  })
  .await;
  let trad_pid2 = root_context.spawn(trad_props2).await;

  println!("Spawning modern BaseActors using new extension method...");

  // Spawn modern BaseActors using the new extension method
  let modern_pid1 = root_context
    .spawn_base_actor(|| ModernGreeter { id: "M1".to_string() })
    .await;

  let modern_pid2 = root_context
    .spawn_base_actor_named(|| ModernGreeter { id: "M2".to_string() }, "modern-greeter-2")
    .await
    .unwrap();

  println!("\nSending messages to all actors...\n");

  // Send messages to all actors (both traditional and modern)
  let msg1 = MessageHandle::new(Greeting {
    name: "Alice".to_string(),
  });
  let msg2 = MessageHandle::new(Greeting {
    name: "Bob".to_string(),
  });
  let msg3 = MessageHandle::new(Greeting {
    name: "Charlie".to_string(),
  });
  let msg4 = MessageHandle::new(Greeting {
    name: "Diana".to_string(),
  });

  root_context.send(trad_pid1.clone(), msg1).await;
  root_context.send(trad_pid2.clone(), msg2).await;
  root_context.send(modern_pid1.clone(), msg3).await;
  root_context.send(modern_pid2.clone(), msg4).await;

  // Wait for messages to be processed
  tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

  println!("\nDemonstrating mixed interaction...");

  // Traditional and modern actors can coexist and interact
  let mixed_msg = MessageHandle::new(Greeting {
    name: "Everyone".to_string(),
  });
  for pid in &[&trad_pid1, &trad_pid2, &modern_pid1, &modern_pid2] {
    root_context.send((*pid).clone(), mixed_msg.clone()).await;
  }

  // Wait for processing
  tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

  println!("\nStopping all actors...");

  // Stop all actors (both types work with the same stop mechanism)
  root_context.stop(&trad_pid1).await;
  root_context.stop(&trad_pid2).await;
  root_context.stop(&modern_pid1).await;
  root_context.stop(&modern_pid2).await;

  // Wait for cleanup
  tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

  println!("\n=== Example completed! ===");
  println!("Both traditional Actor and modern BaseActor work seamlessly together!");
}
