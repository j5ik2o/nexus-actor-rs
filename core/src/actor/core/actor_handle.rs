use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::actor::context::ContextHandle;
use crate::actor::core::actor::Actor;
use crate::actor::core::actor_error::ActorError;
use crate::actor::supervisor::SupervisorStrategyHandle;

#[derive(Debug, Clone)]
pub struct ActorHandle {
  actor: Arc<RwLock<dyn Actor>>,
  type_name: Arc<str>,
}

impl PartialEq for ActorHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.actor, &other.actor)
  }
}

impl Eq for ActorHandle {}

impl std::hash::Hash for ActorHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.actor.as_ref() as *const RwLock<dyn Actor>).hash(state);
  }
}

impl ActorHandle {
  pub fn new_arc(actor: Arc<RwLock<dyn Actor>>, type_name: Arc<str>) -> Self {
    ActorHandle { actor, type_name }
  }

  pub fn new(actor: impl Actor + 'static) -> Self {
    let type_name = Arc::<str>::from(actor.get_type_name());
    ActorHandle {
      actor: Arc::new(RwLock::new(actor)),
      type_name,
    }
  }

  pub fn type_name(&self) -> &str {
    self.type_name.as_ref()
  }

  pub fn type_name_arc(&self) -> Arc<str> {
    self.type_name.clone()
  }
}

#[async_trait]
impl Actor for ActorHandle {
  async fn handle(&mut self, c: ContextHandle) -> Result<(), ActorError> {
    let mut mg = self.actor.write().await;
    mg.handle(c).await
  }

  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let mut mg = self.actor.write().await;
    mg.receive(context_handle).await
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    let mut mg = self.actor.write().await;
    mg.get_supervisor_strategy().await
  }
}
