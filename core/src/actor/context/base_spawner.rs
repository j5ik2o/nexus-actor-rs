use crate::actor::context::{RootContext, SpawnerPart};
use crate::actor::core::{ExtendedPid, SpawnError};
use crate::actor::core_types::{BaseActor, MigrationHelpers};
use async_trait::async_trait;

/// Extension trait for RootContext to spawn BaseActors directly
#[async_trait]
pub trait BaseSpawnerExt {
  /// Spawn a BaseActor using a factory function
  async fn spawn_base_actor<F, B>(&mut self, factory: F) -> ExtendedPid
  where
    F: Fn() -> B + Send + Sync + 'static,
    B: BaseActor + 'static;

  /// Spawn a BaseActor using a factory function with a specific name
  async fn spawn_base_actor_named<F, B>(&mut self, factory: F, name: &str) -> Result<ExtendedPid, SpawnError>
  where
    F: Fn() -> B + Send + Sync + 'static,
    B: BaseActor + 'static;
}

#[async_trait]
impl BaseSpawnerExt for RootContext {
  async fn spawn_base_actor<F, B>(&mut self, factory: F) -> ExtendedPid
  where
    F: Fn() -> B + Send + Sync + 'static,
    B: BaseActor + 'static,
  {
    let props = MigrationHelpers::props_from_base_actor_fn(factory).await;
    self.spawn(props).await
  }

  async fn spawn_base_actor_named<F, B>(&mut self, factory: F, name: &str) -> Result<ExtendedPid, SpawnError>
  where
    F: Fn() -> B + Send + Sync + 'static,
    B: BaseActor + 'static,
  {
    let props = MigrationHelpers::props_from_base_actor_fn(factory).await;
    self.spawn_named(props, name).await
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::SenderPart;
  use crate::actor::core_types::{BaseActor, BaseActorError, BaseContext, Message};
  use crate::actor::message::MessageHandle;
  use std::any::Any;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;

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

  #[derive(Debug)]
  struct TestActor {
    count: Arc<AtomicUsize>,
  }

  #[async_trait]
  impl BaseActor for TestActor {
    async fn handle(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
      let msg = context.get_message().await;
      if msg.to_typed::<TestMessage>().is_some() {
        self.count.fetch_add(1, Ordering::SeqCst);
      }
      Ok(())
    }

    async fn pre_start(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
      println!("TestActor started");
      Ok(())
    }
  }

  #[tokio::test]
  async fn test_spawn_base_actor() {
    let actor_system = ActorSystem::new().await.unwrap();
    let mut root_context = actor_system.get_root_context().await;

    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();

    // Spawn BaseActor using factory
    let pid = root_context
      .spawn_base_actor(move || TestActor {
        count: count_clone.clone(),
      })
      .await;

    // Send a message
    let msg = MessageHandle::new(TestMessage("Hello".to_string()));
    root_context.send(pid.clone(), msg).await;

    // Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Verify
    assert_eq!(count.load(Ordering::SeqCst), 1);
  }

  #[tokio::test]
  async fn test_spawn_base_actor_named() {
    let actor_system = ActorSystem::new().await.unwrap();
    let mut root_context = actor_system.get_root_context().await;

    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();

    // Spawn BaseActor with name
    let result = root_context
      .spawn_base_actor_named(
        move || TestActor {
          count: count_clone.clone(),
        },
        "test-actor",
      )
      .await;
    assert!(result.is_ok());

    let pid = result.unwrap();
    assert!(pid.id().contains("test-actor"));
  }
}
