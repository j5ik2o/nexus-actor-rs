use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::actor::core_types::{BaseActor, BaseActorError, BaseContext, MigratedActor};

/// BaseActor version of ActorHandle
#[derive(Debug, Clone)]
pub struct ActorHandleBase(Arc<RwLock<dyn BaseActor>>);

impl PartialEq for ActorHandleBase {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ActorHandleBase {}

impl std::hash::Hash for ActorHandleBase {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const RwLock<dyn BaseActor>).hash(state);
  }
}

impl ActorHandleBase {
  pub fn new_arc(actor: Arc<RwLock<dyn BaseActor>>) -> Self {
    ActorHandleBase(actor)
  }

  pub fn new(actor: impl BaseActor + 'static) -> Self {
    ActorHandleBase(Arc::new(RwLock::new(actor)))
  }

  /// Convert to traditional Actor
  pub fn into_actor(self) -> MigratedActor<Self> {
    MigratedActor::new(self)
  }
}

#[async_trait]
impl BaseActor for ActorHandleBase {
  async fn handle(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
    let mut mg = self.0.write().await;
    mg.handle(context).await
  }

  async fn pre_start(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
    let mut mg = self.0.write().await;
    mg.pre_start(context).await
  }

  async fn post_start(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
    let mut mg = self.0.write().await;
    mg.post_start(context).await
  }

  async fn pre_stop(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
    let mut mg = self.0.write().await;
    mg.pre_stop(context).await
  }

  async fn post_stop(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
    let mut mg = self.0.write().await;
    mg.post_stop(context).await
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::{SenderPart, SpawnerPart};
  use crate::actor::core_types::{Message, MigrationHelpers};
  use crate::actor::message::MessageHandle;
  use std::any::Any;
  use std::sync::atomic::{AtomicUsize, Ordering};

  #[derive(Debug, Clone)]
  struct CountMessage;

  impl Message for CountMessage {
    fn eq_message(&self, other: &dyn Message) -> bool {
      other.as_any().downcast_ref::<CountMessage>().is_some()
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
      self
    }

    fn get_type_name(&self) -> String {
      "CountMessage".to_string()
    }
  }

  #[derive(Debug)]
  struct CounterActor {
    count: Arc<AtomicUsize>,
  }

  impl CounterActor {
    fn new(count: Arc<AtomicUsize>) -> Self {
      Self { count }
    }
  }

  #[async_trait]
  impl BaseActor for CounterActor {
    async fn handle(&mut self, context: &dyn BaseContext) -> Result<(), BaseActorError> {
      let msg = context.get_message().await;
      if msg.to_typed::<CountMessage>().is_some() {
        self.count.fetch_add(1, Ordering::SeqCst);
        println!("Counter incremented to: {}", self.count.load(Ordering::SeqCst));
      }
      Ok(())
    }
  }

  #[tokio::test]
  async fn test_actor_handle_base() {
    let actor_system = ActorSystem::new().await.unwrap();
    let mut root_context = actor_system.get_root_context().await;

    let count = Arc::new(AtomicUsize::new(0));
    let counter_actor = CounterActor::new(count.clone());

    // Wrap in ActorHandleBase
    let handle_actor = ActorHandleBase::new(counter_actor);

    // Create Props using migration helper
    let props = MigrationHelpers::props_from_base_actor_fn(move || handle_actor.clone()).await;

    // Spawn the actor
    let pid = root_context.spawn(props).await;

    // Send messages
    for _ in 0..3 {
      let msg = MessageHandle::new(CountMessage);
      root_context.send(pid.clone(), msg).await;
    }

    // Give some time for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify count
    assert_eq!(count.load(Ordering::SeqCst), 3);
  }
}
