use nexus_actor_core_rs::actor::actor_system::ActorSystem;
use nexus_actor_core_rs::actor::context::{SpawnerPart, SenderPart, StopperPart};
use nexus_actor_core_rs::actor::core_types::{
    BaseActor, BaseContext, BaseActorError, Message, MigrationHelpers,
};
use nexus_actor_core_rs::actor::message::MessageHandle;
use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};

// Shared counter for demonstration
static MESSAGE_COUNT: AtomicUsize = AtomicUsize::new(0);

// Message types
#[derive(Debug, Clone)]
struct Ping {
    from: String,
}

#[derive(Debug, Clone)]
struct Pong {
    from: String,
    count: usize,
}

impl Message for Ping {
    fn eq_message(&self, other: &dyn Message) -> bool {
        if let Some(other_ping) = other.as_any().downcast_ref::<Ping>() {
            self.from == other_ping.from
        } else {
            false
        }
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
        self
    }

    fn get_type_name(&self) -> String {
        "Ping".to_string()
    }
}

impl Message for Pong {
    fn eq_message(&self, other: &dyn Message) -> bool {
        if let Some(other_pong) = other.as_any().downcast_ref::<Pong>() {
            self.from == other_pong.from && self.count == other_pong.count
        } else {
            false
        }
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
        self
    }

    fn get_type_name(&self) -> String {
        "Pong".to_string()
    }
}

// New-style actor using BaseActor trait
#[derive(Debug)]
struct PingActor {
    name: String,
    pong_count: usize,
}

impl PingActor {
    fn new(name: String) -> Self {
        Self {
            name,
            pong_count: 0,
        }
    }
}

#[async_trait]
impl BaseActor for PingActor {
    async fn handle(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
        let msg = context.get_message().await;
        
        if let Some(pong) = msg.to_typed::<Pong>() {
            self.pong_count += 1;
            println!("{} received Pong #{} from {}", self.name, pong.count, pong.from);
            
            // Respond with another Ping if count is less than 5
            if pong.count < 5 {
                if let Some(sender) = context.get_sender().await {
                    println!("{} sending Ping back to sender", self.name);
                    let ping = MessageHandle::new(Ping {
                        from: self.name.clone(),
                    });
                    context.send(sender.as_ref(), ping).await;
                }
            } else {
                println!("{} finished ping-pong game!", self.name);
            }
        } else {
            println!("{} received unknown message", self.name);
        }
        
        Ok(())
    }
    
    async fn pre_start(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
        println!("{} started", self.name);
        Ok(())
    }
    
    async fn post_stop(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
        println!("{} stopped after {} pongs", self.name, self.pong_count);
        Ok(())
    }
}

// Another new-style actor
#[derive(Debug)]
struct PongActor {
    name: String,
    ping_count: usize,
}

impl PongActor {
    fn new(name: String) -> Self {
        Self {
            name,
            ping_count: 0,
        }
    }
}

#[async_trait]
impl BaseActor for PongActor {
    async fn handle(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
        let msg = context.get_message().await;
        
        if let Some(ping) = msg.to_typed::<Ping>() {
            self.ping_count += 1;
            let count = MESSAGE_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
            println!("{} received Ping from {} (total messages: {})", self.name, ping.from, count);
            
            // Respond with Pong
            if let Some(sender) = context.get_sender().await {
                println!("{} sending Pong back to sender", self.name);
                let pong = MessageHandle::new(Pong {
                    from: self.name.clone(),
                    count: self.ping_count,
                });
                context.send(sender.as_ref(), pong).await;
            } else {
                println!("{} has no sender to respond to", self.name);
            }
        } else {
            println!("{} received unknown message", self.name);
        }
        
        Ok(())
    }
    
    async fn pre_start(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
        println!("{} started", self.name);
        Ok(())
    }
    
    async fn post_stop(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
        println!("{} stopped after {} pings", self.name, self.ping_count);
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    println!("=== Actor Migration Example ===\n");
    
    // Create actor system
    let actor_system = ActorSystem::new().await.unwrap();
    let mut root_context = actor_system.get_root_context().await;
    
    // Use migration helpers to create Props from BaseActors
    println!("Creating actors using migration helpers...");
    let ping_props = MigrationHelpers::props_from_base_actor_fn(|| {
        PingActor::new("PingActor".to_string())
    }).await;
    let pong_props = MigrationHelpers::props_from_base_actor_fn(|| {
        PongActor::new("PongActor".to_string())
    }).await;
    
    // Spawn actors using the traditional actor system
    let ping_pid = root_context.spawn(ping_props).await;
    let pong_pid = root_context.spawn(pong_props).await;
    
    println!("\nStarting ping-pong game...\n");
    
    // Start the ping-pong game
    // First, let's give the PongActor a reference to the PingActor by sending an initial ping
    let initial_ping = MessageHandle::new(Ping {
        from: "PingActor".to_string(),
    });
    
    // Send from ping to pong to establish communication
    root_context.send(pong_pid.clone(), initial_ping).await;
    
    // Let the actors play
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    println!("\nStopping actors...");
    
    // Stop both actors
    root_context.stop(&ping_pid).await;
    root_context.stop(&pong_pid).await;
    
    // Wait for cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    let total_messages = MESSAGE_COUNT.load(Ordering::SeqCst);
    println!("\n=== Example completed! Total messages exchanged: {} ===", total_messages);
}