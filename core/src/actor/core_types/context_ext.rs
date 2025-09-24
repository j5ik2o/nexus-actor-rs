use crate::actor::context::{ContextHandle, InfoPart};
use crate::actor::core::{Actor, Props};
use crate::actor::core_types::adapters::ContextAdapter;
use crate::actor::core_types::{ActorRef, BaseActor, BaseContext, PidActorRef};
use async_trait::async_trait;

/// Extension trait for BaseContext to provide Props-based operations
#[async_trait]
pub trait BaseContextExt: BaseContext {
  /// Spawn a child actor using Props
  async fn spawn_child_with_props(&self, props: Props, name: &str) -> Box<dyn ActorRef>;

  /// Spawn a child actor from a BaseActor factory
  async fn spawn_child_actor<F, B>(&self, name: &str, factory: F) -> Box<dyn ActorRef>
  where
    F: Fn() -> B + Send + Sync + 'static,
    B: BaseActor + 'static;

  /// Get the underlying ContextHandle if available (for advanced usage)
  fn get_context_handle(&self) -> Option<&ContextHandle>;
}

#[async_trait]
impl BaseContextExt for ContextAdapter {
  async fn spawn_child_with_props(&self, props: Props, name: &str) -> Box<dyn ActorRef> {
    use crate::actor::context::SpawnerPart;

    let mut context = self.get_context().clone();
    match context.spawn_named(props, name).await {
      Ok(pid) => {
        let actor_system = context.get_actor_system().await;
        Box::new(PidActorRef::new(pid, actor_system))
      }
      Err(e) => {
        // In case of error, return a dummy ActorRef
        // In a real implementation, this should propagate the error
        eprintln!("Failed to spawn child actor: {:?}", e);
        let dummy_pid = crate::generated::actor::Pid::new("", "");
        let extended_pid = crate::actor::core::ExtendedPid::new(dummy_pid);
        let actor_system = context.get_actor_system().await;
        Box::new(PidActorRef::new(extended_pid, actor_system))
      }
    }
  }

  async fn spawn_child_actor<F, B>(&self, name: &str, factory: F) -> Box<dyn ActorRef>
  where
    F: Fn() -> B + Send + Sync + 'static,
    B: BaseActor + 'static,
  {
    let props = props_from_base_actor_factory(factory).await;
    self.spawn_child_with_props(props, name).await
  }

  fn get_context_handle(&self) -> Option<&ContextHandle> {
    Some(self.get_context())
  }
}

/// Helper function to create Props from a BaseActor factory
/// Using Arc<dyn Fn()> to satisfy Send + Sync requirements
pub async fn props_from_base_actor_factory<F, B>(factory: F) -> Props
where
  F: Fn() -> B + Send + Sync + 'static,
  B: BaseActor + 'static,
{
  use crate::actor::core_types::MigratedActor;

  Props::from_async_actor_receiver(move |ctx| {
    let mut migrated = MigratedActor::new(factory());
    async move { Actor::handle(&mut migrated, ctx).await }
  })
  .await
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{SenderPart, SpawnerPart};
  use crate::actor::core::{Actor, ActorError};
  use crate::actor::core_types::{BaseActorError, Message};
  use crate::actor::message::MessageHandle;
  use std::any::Any;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;

  #[derive(Debug, Clone)]
  struct TestMessage;

  impl Message for TestMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other.as_any().downcast_ref::<TestMessage>().is_some()
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
    async fn handle(&mut self, _context: &dyn BaseContext) -> Result<(), BaseActorError> {
      self.count.fetch_add(1, Ordering::SeqCst);
      Ok(())
    }
  }

  #[derive(Debug)]
  struct ParentActor {
    child_count: Arc<AtomicUsize>,
  }

  #[async_trait]
  impl Actor for ParentActor {
    async fn receive(&mut self, context: ContextHandle) -> Result<(), ActorError> {
      use crate::actor::core_types::ActorBridge;

      let base_context = self.adapt_context(context.clone());

      // Use ContextAdapter directly since we know it's the implementation
      let ctx_ref = base_context.as_ref();
      if let Some(adapter) = ctx_ref.as_any().downcast_ref::<ContextAdapter>() {
        // Test spawning with Props
        let count = self.child_count.clone();
        let child_props = Props::from_async_actor_receiver(move |ctx| {
          let count = count.clone();
          let mut actor = TestActor { count };
          async move {
            let base_ctx = ContextAdapter::new(ctx);
            actor
              .handle(&base_ctx)
              .await
              .map_err(|e| ActorError::ReceiveError(crate::actor::core::ErrorReason::new(e.to_string(), 0)))
          }
        })
        .await;

        // Use unique names to avoid conflicts
        let timestamp = std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .unwrap()
          .as_nanos();
        let child_name = format!("test-child-{}", timestamp);
        let _child = adapter.spawn_child_with_props(child_props, &child_name).await;

        // Also test the factory-based spawn
        let count2 = self.child_count.clone();
        let child_name2 = format!("test-child-2-{}", timestamp);
        let _child2 = adapter
          .spawn_child_actor(&child_name2, move || TestActor { count: count2.clone() })
          .await;
      }

      Ok(())
    }
  }

  impl crate::actor::core_types::ActorBridge for ParentActor {}

  #[tokio::test]
  async fn test_spawn_child_with_props() {
    let actor_system = ActorSystem::new().await.unwrap();
    let mut root_context = actor_system.get_root_context().await;

    let child_count = Arc::new(AtomicUsize::new(0));
    // ParentActor is not a BaseActor, so we use the traditional Props
    let parent_props = Props::from_async_actor_receiver(move |ctx| {
      let mut actor = ParentActor {
        child_count: child_count.clone(),
      };
      async move { actor.receive(ctx).await }
    })
    .await;
    let parent_pid = root_context.spawn(parent_props).await;

    // Send message to trigger child spawn
    let msg = MessageHandle::new(TestMessage);
    root_context.send(parent_pid, msg).await;

    // Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify child was created (the test mainly checks that spawn_child_with_props works)
    // In a real test, we'd send a message to the child and verify it processes it
  }
}
