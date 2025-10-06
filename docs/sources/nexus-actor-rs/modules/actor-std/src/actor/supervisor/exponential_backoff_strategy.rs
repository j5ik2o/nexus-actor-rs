use std::sync::Arc;
use std::time::Duration;

use rand::Rng;

use crate::actor::core::ErrorReason;
use crate::actor::core::RestartStatistics;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::core_adapters::{stats_from_tracker, StdSupervisorAdapter, StdSupervisorContext};
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::supervisor_strategy::{log_failure, Supervisor};
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::actor::core_types::restart::CoreRestartTracker;
use nexus_actor_core_rs::error::ErrorReasonCore;
use nexus_actor_core_rs::supervisor::{
  CoreSupervisor, CoreSupervisorContext, CoreSupervisorStrategy, CoreSupervisorStrategyFuture,
};

#[derive(Debug, Clone)]
pub struct ExponentialBackoffStrategy {
  backoff_window: Duration,
  initial_backoff: Option<Duration>,
}

impl ExponentialBackoffStrategy {
  pub fn new(backoff_window: Duration) -> Self {
    Self {
      backoff_window,
      initial_backoff: None,
    }
  }

  pub fn with_initial_backoff(mut self, initial_backoff: Duration) -> Self {
    self.initial_backoff = Some(initial_backoff);
    self
  }

  pub(crate) async fn set_failure_count(&self, rs: &mut RestartStatistics) {
    if rs.number_of_failures(self.backoff_window).await == 0 {
      rs.reset().await;
    }
    rs.fail().await;
  }
}

impl CoreSupervisorStrategy for ExponentialBackoffStrategy {
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
    let failure_clock = std_ctx.failure_clock();
    let supervisor_handle = std_supervisor.handle();
    let tracker_ref = tracker;
    let child_clone = child.clone();
    let reason_std = ErrorReason::from_core(reason);
    let initial_backoff = self.initial_backoff;

    Box::pin(async move {
      let mut stats = stats_from_tracker(tracker_ref, failure_clock);
      self.set_failure_count(&mut stats).await;

      let failure_count = stats.failure_count().await as u64;
      let base = initial_backoff.map(|v| v.as_nanos()).unwrap_or_default();
      let backoff = failure_count.saturating_mul(base as u64);
      let noise = rand::rng().random_range(0..500);
      let dur = Duration::from_nanos(backoff + noise);

      let scheduler = actor_system.core_runtime().scheduler();
      let actor_system_clone = actor_system.clone();
      let supervisor_clone = supervisor_handle.clone();
      let child_clone_inner = child_clone.clone();
      let reason_clone = reason_std.clone();

      scheduler.schedule_once(
        dur,
        Arc::new(move || {
          let actor_system = actor_system_clone.clone();
          let supervisor = supervisor_clone.clone();
          let child = child_clone_inner.clone();
          let reason = reason_clone.clone();
          Box::pin(async move {
            log_failure(actor_system.clone(), &child, reason.clone(), Directive::Restart).await;
            supervisor.restart_children(&[child]).await;
          })
        }),
      );

      let updated = stats.into_core_tracker().await;
      *tracker_ref = updated;
    })
  }
}

#[cfg(test)]
mod tests;
