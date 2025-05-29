use std::any::Any;

use async_trait::async_trait;

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::ErrorReason;
use crate::actor::core::ExtendedPid;
use crate::actor::core::RestartStatistics;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::supervisor_strategy::{log_failure, Supervisor, SupervisorHandle, SupervisorStrategy};

#[derive(Debug, Clone)]
pub struct RestartingStrategy;

impl RestartingStrategy {
  pub fn new() -> Self {
    RestartingStrategy {}
  }
}

impl Default for RestartingStrategy {
  fn default() -> Self {
    RestartingStrategy::new()
  }
}

impl PartialEq for RestartingStrategy {
  fn eq(&self, _other: &Self) -> bool {
    true
  }
}

impl Eq for RestartingStrategy {}

impl std::hash::Hash for RestartingStrategy {
  fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {
    // nothing to hash
  }
}

#[async_trait]
impl SupervisorStrategy for RestartingStrategy {
  async fn handle_child_failure(
    &self,
    actor_system: ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    _: RestartStatistics,
    reason: ErrorReason,
    _: MessageHandle,
  ) {
    // always restart
    log_failure(actor_system, &child, reason, Directive::Restart).await;
    supervisor.restart_children(&[child]).await
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[cfg(test)]
mod tests;
