use crate::actor::context::{RootContext, SpawnerPart};
use crate::actor::core::{Actor, ExtendedPid, Props, SpawnError};
use async_trait::async_trait;
use std::sync::Arc;

/// Actor トレイト対応の RootContext 拡張
#[async_trait]
pub trait ActorSpawnerExt {
  async fn spawn_actor<F, A>(&mut self, factory: F) -> ExtendedPid
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor + 'static;

  async fn spawn_actor_named<F, A>(&mut self, factory: F, name: &str) -> Result<ExtendedPid, SpawnError>
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor + 'static;
}

#[async_trait]
impl ActorSpawnerExt for RootContext {
  async fn spawn_actor<F, A>(&mut self, factory: F) -> ExtendedPid
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor + 'static, {
    let factory = Arc::new(factory);
    let props = Props::from_async_actor_producer(move |_| {
      let factory = factory.clone();
      async move { factory() }
    })
    .await;
    self.spawn(props).await
  }

  async fn spawn_actor_named<F, A>(&mut self, factory: F, name: &str) -> Result<ExtendedPid, SpawnError>
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor + 'static, {
    let factory = Arc::new(factory);
    let props = Props::from_async_actor_producer(move |_| {
      let factory = factory.clone();
      async move { factory() }
    })
    .await;
    self.spawn_named(props, name).await
  }
}

/// 互換用 (非推奨): 旧 BaseActor API を呼び出すコードのためのブリッジ
#[deprecated(
  since = "1.2.0",
  note = "BaseActor 系 API は廃止されました。ActorSpawnerExt::spawn_actor を利用してください。"
)]
#[async_trait]
pub trait BaseSpawnerExt {
  async fn spawn_base_actor<F, A>(&mut self, factory: F) -> ExtendedPid
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor + 'static;

  async fn spawn_base_actor_named<F, A>(&mut self, factory: F, name: &str) -> Result<ExtendedPid, SpawnError>
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor + 'static;
}

#[allow(deprecated)]
#[async_trait]
impl BaseSpawnerExt for RootContext {
  async fn spawn_base_actor<F, A>(&mut self, factory: F) -> ExtendedPid
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor + 'static, {
    self.spawn_actor(factory).await
  }

  async fn spawn_base_actor_named<F, A>(&mut self, factory: F, name: &str) -> Result<ExtendedPid, SpawnError>
  where
    F: Fn() -> A + Send + Sync + 'static,
    A: Actor + 'static, {
    self.spawn_actor_named(factory, name).await
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{ContextHandle, MessagePart, SenderPart};
  use crate::actor::core::{Actor, ActorError};
  use crate::actor::message::{Message, MessageHandle};
  use std::any::Any;
  use std::sync::atomic::{AtomicUsize, Ordering};
  use std::sync::Arc;

  #[derive(Debug, Clone)]
  struct TestMessage(String);

  impl Message for TestMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other
        .as_any()
        .downcast_ref::<TestMessage>()
        .map(|msg| msg.0 == self.0)
        .unwrap_or(false)
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
  impl Actor for TestActor {
    async fn receive(&mut self, context: ContextHandle) -> Result<(), ActorError> {
      let message = context.get_message_handle_opt().await.expect("message not found");
      if message.to_typed::<TestMessage>().is_some() {
        self.count.fetch_add(1, Ordering::SeqCst);
      }
      Ok(())
    }
  }

  #[tokio::test]
  async fn test_spawn_actor() {
    let actor_system = ActorSystem::new().await.unwrap();
    let mut root_context = actor_system.get_root_context().await;

    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();

    // Spawn Actor using factory
    let pid = root_context
      .spawn_actor(move || TestActor {
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
  async fn test_spawn_actor_named() {
    let actor_system = ActorSystem::new().await.unwrap();
    let mut root_context = actor_system.get_root_context().await;

    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();

    // Spawn Actor with name
    let result = root_context
      .spawn_actor_named(
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
