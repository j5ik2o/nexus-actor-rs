use crate::actor::core::ErrorReason;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::core_adapters::{StdSupervisorAdapter, StdSupervisorContext};
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::supervisor_strategy::{log_failure, record_supervisor_metrics, Supervisor};
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::actor::core_types::restart::CoreRestartTracker;
use nexus_actor_core_rs::error::ErrorReasonCore;
use nexus_actor_core_rs::supervisor::{
  CoreSupervisor, CoreSupervisorContext, CoreSupervisorStrategy, CoreSupervisorStrategyFuture,
};

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

impl CoreSupervisorStrategy for RestartingStrategy {
  fn handle_child_failure<'a>(
    &'a self,
    ctx: &'a dyn CoreSupervisorContext,
    supervisor: &'a dyn CoreSupervisor,
    child: CorePid,
    tracker: &'a mut CoreRestartTracker,
    reason: ErrorReasonCore,
    _message_handle: MessageHandle,
  ) -> CoreSupervisorStrategyFuture<'a> {
    let std_ctx = ctx
      .as_any()
      .downcast_ref::<StdSupervisorContext>()
      .expect("StdSupervisorContext expected");
    let std_supervisor = supervisor
      .as_any()
      .downcast_ref::<StdSupervisorAdapter>()
      .expect("StdSupervisorAdapter expected");

    let actor_system = std_ctx.actor_system();
    let supervisor_handle = std_supervisor.handle();
    let reason_std = ErrorReason::from_core(reason);
    let _ = tracker;
    let child_clone = child.clone();

    Box::pin(async move {
      record_supervisor_metrics(&supervisor_handle, "restarting", "restart", &child_clone, Vec::new());
      log_failure(
        actor_system.clone(),
        &child_clone,
        reason_std.clone(),
        Directive::Restart,
      )
      .await;
      let children = [child_clone.clone()];
      supervisor_handle.restart_children(&children).await;
    })
  }
}

#[cfg(test)]
mod tests;
