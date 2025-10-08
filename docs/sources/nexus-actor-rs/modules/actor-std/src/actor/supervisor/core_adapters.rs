use std::time::Instant;

use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::actor::core_types::restart::FailureClock;
use nexus_actor_core_rs::error::ErrorReasonCore;
use nexus_actor_core_rs::runtime::CoreRuntime;
use nexus_actor_core_rs::supervisor::{
  CoreSupervisor, CoreSupervisorContext, CoreSupervisorDirective, CoreSupervisorFuture,
};

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::{ErrorReason, RestartStatistics};
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::supervisor_strategy::SupervisorHandle;

use nexus_actor_core_rs::actor::core_types::restart::CoreRestartTracker;
use nexus_utils_std_rs::runtime::sync::InstantFailureClock;

pub struct StdSupervisorContext {
  actor_system: ActorSystem,
  core_runtime: CoreRuntime,
  failure_clock: Option<InstantFailureClock>,
  anchor: Instant,
}

impl StdSupervisorContext {
  pub fn new(actor_system: ActorSystem) -> Self {
    let core_runtime = actor_system.core_runtime();
    Self::with_runtime(actor_system, core_runtime)
  }

  pub fn with_runtime(actor_system: ActorSystem, core_runtime: CoreRuntime) -> Self {
    let failure_clock = core_runtime
      .failure_clock()
      .and_then(|clock| clock.as_any().downcast_ref::<InstantFailureClock>().cloned());
    Self {
      actor_system,
      core_runtime,
      failure_clock,
      anchor: Instant::now(),
    }
  }

  pub fn actor_system(&self) -> ActorSystem {
    self.actor_system.clone()
  }

  pub fn core_runtime(&self) -> CoreRuntime {
    self.core_runtime.clone()
  }

  pub fn failure_clock(&self) -> Option<InstantFailureClock> {
    self.failure_clock.clone()
  }
}

impl CoreSupervisorContext for StdSupervisorContext {
  fn now(&self) -> u64 {
    fn saturating_millis(duration: std::time::Duration) -> u64 {
      let millis = duration.as_millis();
      if millis > u64::MAX as u128 {
        u64::MAX
      } else {
        millis as u64
      }
    }

    if let Some(clock) = &self.failure_clock {
      saturating_millis(clock.now())
    } else {
      saturating_millis(self.anchor.elapsed())
    }
  }

  fn as_any(&self) -> &(dyn std::any::Any + Send + Sync + 'static) {
    self
  }
}

pub fn stats_from_tracker(
  tracker: &CoreRestartTracker,
  failure_clock: Option<InstantFailureClock>,
) -> RestartStatistics {
  if let Some(clock) = failure_clock {
    RestartStatistics::from_core_tracker_with_clock(tracker.clone(), clock)
  } else {
    let anchor = tracker.samples().last().copied().unwrap_or_default();
    RestartStatistics::from_core_tracker_with_anchor(tracker.clone(), anchor)
  }
}

pub struct StdSupervisorAdapter {
  handle: SupervisorHandle,
}

impl StdSupervisorAdapter {
  pub fn new(handle: SupervisorHandle) -> Self {
    Self { handle }
  }

  pub fn handle(&self) -> SupervisorHandle {
    self.handle.clone()
  }

  pub async fn stop_children_core(&self, targets: &[CorePid]) {
    let supervisor = self.handle.get_supervisor();
    supervisor.stop_children(targets).await;
  }
}

impl CoreSupervisor for StdSupervisorAdapter {
  fn children<'a>(&'a self) -> CoreSupervisorFuture<'a, Vec<CorePid>> {
    let supervisor = self.handle.get_supervisor();
    Box::pin(async move { supervisor.get_children().await })
  }

  fn apply_directive<'a>(
    &'a self,
    directive: CoreSupervisorDirective,
    targets: &'a [CorePid],
    reason: ErrorReasonCore,
    message: MessageHandle,
  ) -> CoreSupervisorFuture<'a, ()> {
    let supervisor = self.handle.get_supervisor();
    let reason_std = ErrorReason::from_core(reason);
    let targets_vec = targets.to_vec();
    Box::pin(async move {
      match directive {
        CoreSupervisorDirective::Resume => supervisor.resume_children(&targets_vec).await,
        CoreSupervisorDirective::Restart => supervisor.restart_children(&targets_vec).await,
        CoreSupervisorDirective::Stop => supervisor.stop_children(&targets_vec).await,
        CoreSupervisorDirective::Escalate => supervisor.escalate_failure(reason_std, message).await,
      }
    })
  }

  fn escalate<'a>(&'a self, reason: ErrorReasonCore, message: MessageHandle) -> CoreSupervisorFuture<'a, ()> {
    let supervisor = self.handle.get_supervisor();
    let reason_std = ErrorReason::from_core(reason);
    Box::pin(async move { supervisor.escalate_failure(reason_std, message).await })
  }

  fn as_any(&self) -> &(dyn std::any::Any + Send + Sync + 'static) {
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use nexus_utils_std_rs::runtime::sync::tokio_core_runtime;
  use std::time::Duration;

  #[tokio::test]
  async fn std_supervisor_context_now_reflects_failure_clock_progress() {
    let system = ActorSystem::new().await.expect("actor system");
    let factory = tokio_core_runtime();
    let context = StdSupervisorContext::with_runtime(system.clone(), runtime);

    let before = context.now();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let after = context.now();

    assert!(after >= before + 10, "context.now should advance with runtime clock");

    drop(system);
  }
}
