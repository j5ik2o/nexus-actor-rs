use std::any::Any;
use std::sync::Arc;

use crate::actor::actor::actor_inner_error::ActorInnerError;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::supervisor::supervisor_strategy::{SupervisorHandle, SupervisorStrategy};
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct SupervisorStrategyHandle(Arc<dyn SupervisorStrategy>);

impl PartialEq for SupervisorStrategyHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for SupervisorStrategyHandle {}

impl std::hash::Hash for SupervisorStrategyHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn SupervisorStrategy).hash(state);
  }
}

impl SupervisorStrategyHandle {
  pub fn new_arc(s: Arc<dyn SupervisorStrategy>) -> Self {
    if s.as_any().downcast_ref::<SupervisorStrategyHandle>().is_some() {
      panic!("SupervisorStrategyHandle can't be used as a strategy, {:?}", s)
    }
    Self(s)
  }

  pub fn new(s: impl SupervisorStrategy + 'static) -> Self {
    if s.as_any().downcast_ref::<SupervisorStrategyHandle>().is_some() {
      panic!("SupervisorStrategyHandle can't be used as a strategy, {:?}", s)
    }
    Self(Arc::new(s))
  }
}

#[async_trait]
impl SupervisorStrategy for SupervisorStrategyHandle {
  async fn handle_child_failure(
    &self,
    actor_system: &ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    rs: RestartStatistics,
    reason: ActorInnerError,
    message: MessageHandle,
  ) {
    self
      .0
      .handle_child_failure(actor_system, supervisor, child, rs, reason, message)
      .await
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}
