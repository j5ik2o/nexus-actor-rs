use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::actor::core::actor::Actor;
use crate::actor::core::actor_error::ActorError;
use crate::actor::context::ContextHandle;
use crate::actor::supervisor::SupervisorStrategyHandle;

#[derive(Debug, Clone)]
pub struct ActorHandle(Arc<RwLock<dyn Actor>>);

impl PartialEq for ActorHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ActorHandle {}

impl std::hash::Hash for ActorHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const RwLock<dyn Actor>).hash(state);
  }
}

impl ActorHandle {
  pub fn new_arc(actor: Arc<RwLock<dyn Actor>>) -> Self {
    ActorHandle(actor)
  }

  pub fn new(actor: impl Actor + 'static) -> Self {
    ActorHandle(Arc::new(RwLock::new(actor)))
  }
}

#[async_trait]
impl Actor for ActorHandle {
  async fn handle(&mut self, c: ContextHandle) -> Result<(), ActorError> {
    let mut mg = self.0.write().await;
    mg.handle(c).await
  }

  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let mut mg = self.0.write().await;
    mg.receive(context_handle).await
  }

  async fn get_supervisor_strategy(&mut self) -> Option<SupervisorStrategyHandle> {
    let mut mg = self.0.write().await;
    mg.get_supervisor_strategy().await
  }
}
