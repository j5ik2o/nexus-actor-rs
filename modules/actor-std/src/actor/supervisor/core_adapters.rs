use std::any::Any;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::error::ErrorReasonCore;
use nexus_actor_core_rs::supervisor::{
  CoreSupervisor, CoreSupervisorContext, CoreSupervisorDirective, CoreSupervisorFuture, CoreSupervisorStrategy,
  CoreSupervisorStrategyFuture,
};

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::{ErrorReason, RestartStatistics};
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::supervisor_strategy::{SupervisorHandle, SupervisorStrategy};
use crate::actor::supervisor::supervisor_strategy_handle::SupervisorStrategyHandle;

use nexus_actor_core_rs::actor::core_types::restart::CoreRestartTracker;

pub struct StdSupervisorContext {
  actor_system: ActorSystem,
  anchor: Instant,
}

impl StdSupervisorContext {
  pub fn new(actor_system: ActorSystem) -> Self {
    Self {
      actor_system,
      anchor: Instant::now(),
    }
  }

  pub fn actor_system(&self) -> ActorSystem {
    self.actor_system.clone()
  }
}

impl CoreSupervisorContext for StdSupervisorContext {
  fn now(&self) -> u64 {
    self.anchor.elapsed().as_millis() as u64
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
}

pub struct StdSupervisorStrategyAdapter {
  inner: Arc<dyn SupervisorStrategy>,
}

impl StdSupervisorStrategyAdapter {
  pub fn new(inner: SupervisorStrategyHandle) -> Self {
    Self {
      inner: Arc::new(inner) as Arc<dyn SupervisorStrategy>,
    }
  }

  pub fn with_arc(inner: Arc<dyn SupervisorStrategy>) -> Self {
    Self { inner }
  }
}

#[async_trait]
impl CoreSupervisorStrategy for StdSupervisorStrategyAdapter {
  fn handle_child_failure<'a>(
    &'a self,
    ctx: &'a dyn CoreSupervisorContext,
    supervisor: &'a dyn CoreSupervisor,
    child: CorePid,
    tracker: &'a mut CoreRestartTracker,
    reason: ErrorReasonCore,
    message: MessageHandle,
  ) -> CoreSupervisorStrategyFuture<'a> {
    let std_ctx = (ctx as &dyn Any)
      .downcast_ref::<StdSupervisorContext>()
      .expect("StdSupervisorContext expected");
    let std_supervisor = (supervisor as &dyn Any)
      .downcast_ref::<StdSupervisorAdapter>()
      .expect("StdSupervisorAdapter expected");

    let actor_system = std_ctx.actor_system();
    let supervisor_handle = std_supervisor.handle();
    let strategy = self.inner.clone();
    let tracker_ref = tracker;
    Box::pin(async move {
      let tracker_clone = tracker_ref.clone();
      let stats = RestartStatistics::from_core_tracker(tracker_clone);
      strategy
        .handle_child_failure(
          actor_system.clone(),
          supervisor_handle.clone(),
          child.clone(),
          stats.clone(),
          ErrorReason::from_core(reason),
          message,
        )
        .await;
      let updated = stats.into_core_tracker().await;
      *tracker_ref = updated;
    })
  }
}
