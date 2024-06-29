use crate::actor::actor_system::ActorSystem;
use crate::actor::directive::Directive;
use crate::actor::message::MessageHandle;
use crate::actor::pid::ExtendedPid;
use crate::actor::restart_statistics::RestartStatistics;
use crate::actor::supervisor_strategy::{log_failure, Supervisor, SupervisorHandle, SupervisorStrategy};
use crate::actor::ReasonHandle;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct RestartingStrategy {}

impl RestartingStrategy {
  pub fn new() -> Self {
    RestartingStrategy {}
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
  async fn handle_failure(
    &self,
    actor_system: &ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    rs: RestartStatistics,
    reason: ReasonHandle,
    message: MessageHandle,
  ) {
    // always restart
    log_failure(actor_system, &child, reason, Directive::Restart).await;
    supervisor.restart_children(&[child]).await
  }
}
