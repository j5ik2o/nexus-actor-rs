use crate::actor::core::ActorReceiver;
use crate::actor::core_types::{BaseActor, BaseActorError, BaseContext, MigratedActor};
use async_trait::async_trait;
use nexus_actor_utils_rs::collections::Stack;
use std::sync::Arc;
use tokio::sync::RwLock;

/// BaseActor version of ActorBehavior
#[derive(Debug, Clone)]
pub struct ActorBehaviorBase {
  stack: Arc<RwLock<Stack<ActorReceiver>>>,
}

impl ActorBehaviorBase {
  pub fn new() -> Self {
    Self {
      stack: Arc::new(RwLock::new(Stack::new())),
    }
  }

  pub async fn reset(&mut self, receiver: ActorReceiver) {
    let mut mg = self.stack.write().await;
    mg.clear();
    mg.push(receiver);
  }

  pub async fn become_stacked(&mut self, receiver: ActorReceiver) {
    let mut mg = self.stack.write().await;
    mg.push(receiver);
  }

  pub async fn un_become_stacked(&mut self) {
    let mut mg = self.stack.write().await;
    mg.pop();
  }

  pub async fn clear(&mut self) {
    let mut mg = self.stack.write().await;
    mg.clear();
  }

  /// Convert to traditional Actor
  pub fn into_actor(self) -> MigratedActor<Self> {
    MigratedActor::new(self)
  }
}

impl Default for ActorBehaviorBase {
  fn default() -> Self {
    ActorBehaviorBase::new()
  }
}

#[async_trait]
impl BaseActor for ActorBehaviorBase {
  async fn handle(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
    // For now, we'll just log when behaviors would be executed
    // In a real migration, we'd need to refactor ActorReceiver to work with BaseContext
    if let Some(_behavior) = {
      let mg = self.stack.read().await;
      mg.peek()
    } {
      // Log that we would execute the behavior
      tracing::debug!("ActorBehaviorBase would execute stacked behavior");
    } else {
      let self_ref = context.self_ref();
      tracing::error!("empty behavior called: pid = {}", self_ref.get_id());
    }

    Ok(())
  }
}

static_assertions::assert_impl_all!(ActorBehaviorBase: Send, Sync);

#[cfg(test)]
mod migration_tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{SenderPart, SpawnerPart};
  use crate::actor::core::ActorError;
  use crate::actor::core_types::{Message, MigrationHelpers};
  use crate::actor::message::MessageHandle;
  use std::any::Any;

  #[derive(Debug, Clone)]
  struct TestMessage(String);

  impl Message for TestMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      if let Some(other_msg) = other.as_any().downcast_ref::<TestMessage>() {
        self.0 == other_msg.0
      } else {
        false
      }
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }

    fn get_type_name(&self) -> String {
      "TestMessage".to_string()
    }
  }

  #[tokio::test]
  async fn test_actor_behavior_base_migration() {
    let actor_system = ActorSystem::new().await.unwrap();
    let mut root_context = actor_system.get_root_context().await;

    // Create behavior actor
    let mut behavior_actor = ActorBehaviorBase::new();

    // Add a test behavior
    let test_receiver = ActorReceiver::new(|_ctx| async move {
      println!("Test behavior executed!");
      Ok::<(), ActorError>(())
    });
    behavior_actor.become_stacked(test_receiver).await;

    // Create Props using migration helper
    let props = MigrationHelpers::props_from_base_actor_fn(move || behavior_actor.clone()).await;

    // Spawn the actor
    let pid = root_context.spawn(props).await;

    // Send a test message
    let msg = MessageHandle::new(TestMessage("Hello".to_string()));
    root_context.send(pid.clone(), msg).await;

    // Give some time for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
  }
}
