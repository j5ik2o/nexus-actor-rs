use crate::actor::context::ContextHandle;
use crate::actor::supervisor_strategy::SupervisorStrategyHandle;
use async_trait::async_trait;
use std::fmt::Debug;
use std::sync::Arc;

include!(concat!(env!("OUT_DIR"), "/actor.rs"));

#[async_trait]
pub trait Actor: Debug + Send + Sync + 'static {
  async fn receive(&self, c: ContextHandle);
  fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    None
  }
}

#[derive(Debug, Clone)]
pub struct ActorHandle(Arc<dyn Actor>);

impl PartialEq for ActorHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ActorHandle {}

impl std::hash::Hash for ActorHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Actor).hash(state);
  }
}

impl ActorHandle {
  pub fn new(actor: Arc<dyn Actor>) -> Self {
    ActorHandle(actor)
  }
}

#[async_trait]
impl Actor for ActorHandle {
  async fn receive(&self, c: ContextHandle) {
    self.0.receive(c).await;
  }
}
