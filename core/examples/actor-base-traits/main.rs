use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{ContextHandle, SpawnerPart, SenderPart, StopperPart};
use nexus_actor_core_rs::actor::core::{Actor, Props, ActorError, ErrorReason};
use nexus_actor_core_rs::actor::core_types::{
    ActorBridge, BaseActor, BaseContext, BaseActorError, Message,
};
use nexus_actor_core_rs::actor::message::MessageHandle;
use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;

// Simple message type
#[derive(Debug, Clone)]
struct Hello {
    name: String,
}

impl Message for Hello {
    fn eq_message(&self, other: &dyn Message) -> bool {
        if let Some(other_hello) = other.as_any().downcast_ref::<Hello>() {
            self.name == other_hello.name
        } else {
            false
        }
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
        self
    }

    fn get_type_name(&self) -> String {
        "Hello".to_string()
    }
}

// Actor using new base traits
#[derive(Debug)]
struct GreeterActor;

#[async_trait]
impl BaseActor for GreeterActor {
    async fn handle(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
        let msg = context.get_message().await;
        if let Some(hello) = msg.to_typed::<Hello>() {
            println!("Hello, {}! (from base actor)", hello.name);
        }
        Ok(())
    }

    async fn pre_start(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
        println!("GreeterActor starting!");
        Ok(())
    }

    async fn post_stop(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
        println!("GreeterActor stopped!");
        Ok(())
    }
}

// Traditional actor that bridges to base actor
#[derive(Debug)]
struct BridgedGreeter;

#[async_trait]
impl Actor for BridgedGreeter {
    async fn receive(&mut self, context: ContextHandle) -> Result<(), ActorError> {
        // Use the bridge to convert context
        let base_context = self.adapt_context(context.clone());
        
        // Create a temporary base actor and delegate to it
        let mut base_actor = GreeterActor;
        base_actor.handle(base_context.as_ref()).await
            .map_err(|e| ActorError::ReceiveError(ErrorReason::new(e.to_string(), 0)))?;
        
        Ok(())
    }
}

impl ActorBridge for BridgedGreeter {}

#[tokio::main]
async fn main() {
    // Create actor system
    let actor_system = ActorSystem::new().await.unwrap();

    // Create actor using traditional system
    let props = Props::from_async_actor_receiver(|ctx| {
        let mut actor = BridgedGreeter;
        async move { actor.receive(ctx).await }
    })
    .await;

    let mut root_context = actor_system.get_root_context().await;
    let pid = root_context.spawn(props).await;

    // Send message using traditional system
    let msg = MessageHandle::new(Hello {
        name: "World".to_string(),
    });
    
    root_context.send(pid.clone(), msg).await;

    // Wait a bit for the message to be processed
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Stop the actor
    root_context.stop(&pid).await;

    // Wait a bit before shutting down
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    println!("Example completed!");
}