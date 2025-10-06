use std::sync::Arc;
use std::time::Duration;

use crate::actor::core::ErrorReason;
use crate::actor::core::RestartStatistics;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::core_adapters::{stats_from_tracker, StdSupervisorAdapter, StdSupervisorContext};
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::supervisor_strategy::{log_failure, record_supervisor_metrics, Decider, Supervisor};
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::actor::core_types::restart::CoreRestartTracker;
use nexus_actor_core_rs::error::ErrorReasonCore;
use nexus_actor_core_rs::supervisor::{
  CoreSupervisor, CoreSupervisorContext, CoreSupervisorStrategy, CoreSupervisorStrategyFuture,
};

pub async fn default_decider(_: ErrorReason) -> Directive {
  Directive::Restart
}

#[derive(Debug, Clone)]
pub struct OneForOneStrategy {
  max_nr_of_retries: u32,
  pub(crate) within_duration: Duration,
  decider: Arc<Decider>,
}

impl OneForOneStrategy {
  pub fn new(max_nr_of_retries: u32, within_duration: Duration) -> Self {
    OneForOneStrategy {
      max_nr_of_retries,
      within_duration,
      decider: Arc::new(Decider::new(default_decider)),
    }
  }

  pub fn with_decider<F, Fut>(mut self, decider: F) -> Self
  where
    F: Fn(ErrorReason) -> Fut + Send + Sync + 'static,
    Fut: futures::future::Future<Output = Directive> + Send + 'static, {
    self.decider = Arc::new(Decider::new(decider));
    self
  }

  pub(crate) async fn should_stop(&self, rs: &mut RestartStatistics) -> bool {
    tracing::debug!(
      "OneForOneStrategy::should_stop: max_retries = {}, failure_count = {}",
      self.max_nr_of_retries,
      rs.failure_count().await
    );
    if self.max_nr_of_retries == 0 {
      tracing::debug!("OneForOneStrategy::should_stop: restart");
      true
    } else {
      rs.fail().await;
      if rs.number_of_failures(self.within_duration).await > self.max_nr_of_retries {
        tracing::debug!("OneForOneStrategy::should_stop: stop");
        rs.reset().await;
        true
      } else {
        tracing::debug!("OneForOneStrategy::should_stop: restart");
        false
      }
    }
  }
}

impl PartialEq for OneForOneStrategy {
  fn eq(&self, other: &Self) -> bool {
    self.max_nr_of_retries == other.max_nr_of_retries
      && self.within_duration == other.within_duration
      && self.decider == other.decider
  }
}

impl Eq for OneForOneStrategy {}

impl std::hash::Hash for OneForOneStrategy {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.max_nr_of_retries.hash(state);
    self.within_duration.hash(state);
    self.decider.hash(state);
  }
}

impl CoreSupervisorStrategy for OneForOneStrategy {
  fn handle_child_failure<'a>(
    &'a self,
    ctx: &'a dyn CoreSupervisorContext,
    supervisor: &'a dyn CoreSupervisor,
    child: CorePid,
    tracker: &'a mut CoreRestartTracker,
    reason: ErrorReasonCore,
    message_handle: MessageHandle,
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
    let decider = self.decider.clone();
    let within_duration = self.within_duration;
    let max_nr_of_retries = self.max_nr_of_retries;
    let tracker_ref = tracker;
    let child_clone = child.clone();
    let message_clone = message_handle.clone();
    let reason_std = ErrorReason::from_core(reason);

    Box::pin(async move {
      let mut stats = stats_from_tracker(tracker_ref, failure_clock);
      tracing::debug!(
        "OneForOneStrategy::handle_child_failure: child = {:?}, rs = {:?}, message = {:?}",
        child_clone.id(),
        stats,
        message_clone
      );

      let record_decision = |decision: &str| {
        record_supervisor_metrics(&supervisor_handle, "one_for_one", decision, &child_clone, Vec::new());
      };

      let directive = decider.run(reason_std.clone()).await;
      match directive {
        Directive::Resume => {
          record_decision("resume");
          tracing::debug!(
            "OneForOneStrategy::handle_child_failure: Resume: child = {:?}, rs = {:?}, message = {:?}",
            child_clone.id(),
            stats,
            message_clone
          );
          log_failure(actor_system.clone(), &child_clone, reason_std.clone(), directive).await;
          let children = [child_clone.clone()];
          supervisor_handle.resume_children(&children).await;
        }
        Directive::Restart => {
          tracing::debug!(
            "OneForOneStrategy::handle_child_failure: Restart: child = {:?}, rs = {:?}, message = {:?}",
            child_clone.id(),
            stats,
            message_clone
          );
          let mut rs = stats.clone();
          if max_nr_of_retries == 0 {
            record_decision("stop_after_restart_attempt");
            log_failure(actor_system.clone(), &child_clone, reason_std.clone(), Directive::Stop).await;
            let children = [child_clone.clone()];
            supervisor_handle.stop_children(&children).await;
            rs.reset().await;
            stats = rs;
          } else {
            rs.fail().await;
            if rs.number_of_failures(within_duration).await > max_nr_of_retries {
              record_decision("stop_after_restart_attempt");
              log_failure(actor_system.clone(), &child_clone, reason_std.clone(), Directive::Stop).await;
              let children = [child_clone.clone()];
              supervisor_handle.stop_children(&children).await;
              rs.reset().await;
            } else {
              record_decision("restart");
              log_failure(
                actor_system.clone(),
                &child_clone,
                reason_std.clone(),
                Directive::Restart,
              )
              .await;
              let children = [child_clone.clone()];
              supervisor_handle.restart_children(&children).await;
            }
            stats = rs;
          }
        }
        Directive::Stop => {
          record_decision("stop");
          tracing::debug!(
            "OneForOneStrategy::handle_child_failure: Stop: child = {:?}, rs = {:?}, message = {:?}",
            child_clone.id(),
            stats,
            message_clone
          );
          log_failure(actor_system.clone(), &child_clone, reason_std.clone(), directive).await;
          let children = [child_clone.clone()];
          supervisor_handle.stop_children(&children).await;
        }
        Directive::Escalate => {
          record_decision("escalate");
          tracing::debug!(
            "OneForOneStrategy::handle_child_failure: Escalate: child = {:?}, rs = {:?}, message = {:?}",
            child_clone.id(),
            stats,
            message_clone
          );
          supervisor_handle
            .escalate_failure(reason_std.clone(), message_clone.clone())
            .await;
        }
      }

      let updated = stats.into_core_tracker().await;
      *tracker_ref = updated;
    })
  }
}

#[cfg(test)]
mod tests;
