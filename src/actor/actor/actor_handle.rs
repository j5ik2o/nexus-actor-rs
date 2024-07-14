use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::actor::actor::actor::Actor;
use crate::actor::actor::actor_error::ActorError;
use crate::actor::context::context_handle::ContextHandle;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;

#[derive(Debug, Clone)]
pub struct ActorHandle(Arc<Mutex<dyn Actor>>);

impl PartialEq for ActorHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ActorHandle {}

impl std::hash::Hash for ActorHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const Mutex<dyn Actor>).hash(state);
  }
}

impl ActorHandle {
  pub fn new_arc(actor: Arc<Mutex<dyn Actor>>) -> Self {
    ActorHandle(actor)
  }

  pub fn new(actor: impl Actor + 'static) -> Self {
    ActorHandle(Arc::new(Mutex::new(actor)))
  }
}

#[async_trait]
impl Actor for ActorHandle {
  async fn handle(&mut self, c: ContextHandle) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.handle(c).await
  }

  async fn receive(&mut self, context_handle: ContextHandle) -> Result<(), ActorError> {
    let mut mg = self.0.lock().await;
    mg.receive(context_handle).await
  }

  async fn get_supervisor_strategy(&self) -> Option<SupervisorStrategyHandle> {
    let mg = self.0.lock().await;
    mg.get_supervisor_strategy().await
  }
}
