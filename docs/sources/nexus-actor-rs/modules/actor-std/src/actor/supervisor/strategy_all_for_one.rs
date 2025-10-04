use opentelemetry::KeyValue;
use std::sync::Arc;
use std::time::Duration;

use crate::actor::core::ErrorReason;
use crate::actor::core::RestartStatistics;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::core_adapters::{stats_from_tracker, StdSupervisorAdapter, StdSupervisorContext};
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::strategy_one_for_one::default_decider;
use crate::actor::supervisor::supervisor_strategy::{log_failure, record_supervisor_metrics, Decider, Supervisor};
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::actor::core_types::restart::CoreRestartTracker;
use nexus_actor_core_rs::error::ErrorReasonCore;
use nexus_actor_core_rs::supervisor::{
  CoreSupervisor, CoreSupervisorContext, CoreSupervisorStrategy, CoreSupervisorStrategyFuture,
};

#[derive(Debug, Clone)]
pub struct AllForOneStrategy {
  max_nr_of_retries: u32,
  within_duration: Duration,
  decider: Arc<Decider>,
}

impl AllForOneStrategy {
  pub fn new(max_nr_of_retries: u32, within_duration: Duration) -> Self {
    AllForOneStrategy {
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

  async fn should_stop(&self, rs: &mut RestartStatistics) -> bool {
    if self.max_nr_of_retries == 0 {
      true
    } else {
      rs.fail().await;
      if rs.number_of_failures(self.within_duration).await > self.max_nr_of_retries {
        rs.reset().await;
        true
      } else {
        false
      }
    }
  }
}

impl CoreSupervisorStrategy for AllForOneStrategy {
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
      let record_decision = |decision: &str, affected_children: usize| {
        record_supervisor_metrics(
          &supervisor_handle,
          "all_for_one",
          decision,
          &child_clone,
          vec![KeyValue::new("supervisor.affected_children", affected_children as i64)],
        );
      };

      let directive = decider.run(reason_std.clone()).await;
      match directive {
        Directive::Resume => {
          record_decision("resume", 1);
          log_failure(actor_system.clone(), &child_clone, reason_std.clone(), directive).await;
          let children = [child_clone.clone()];
          supervisor_handle.resume_children(&children).await;
        }
        Directive::Restart => {
          let children = supervisor_handle.get_children().await;
          let mut rs = stats.clone();
          if max_nr_of_retries == 0 {
            record_decision("stop_all", children.len());
            log_failure(actor_system.clone(), &child_clone, reason_std.clone(), Directive::Stop).await;
            supervisor_handle.stop_children(&children).await;
            rs.reset().await;
          } else {
            rs.fail().await;
            if rs.number_of_failures(within_duration).await > max_nr_of_retries {
              record_decision("stop_all", children.len());
              log_failure(actor_system.clone(), &child_clone, reason_std.clone(), Directive::Stop).await;
              supervisor_handle.stop_children(&children).await;
              rs.reset().await;
            } else {
              record_decision("restart_all", children.len());
              log_failure(
                actor_system.clone(),
                &child_clone,
                reason_std.clone(),
                Directive::Restart,
              )
              .await;
              supervisor_handle.restart_children(&children).await;
            }
          }
          stats = rs;
        }
        Directive::Stop => {
          let children = supervisor_handle.get_children().await;
          record_decision("stop_all_explicit", children.len());
          log_failure(actor_system.clone(), &child_clone, reason_std.clone(), directive).await;
          supervisor_handle.stop_children(&children).await;
        }
        Directive::Escalate => {
          record_decision("escalate", 0);
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

impl PartialEq for AllForOneStrategy {
  fn eq(&self, other: &Self) -> bool {
    self.max_nr_of_retries == other.max_nr_of_retries
      && self.within_duration == other.within_duration
      && self.decider == other.decider
  }
}

impl Eq for AllForOneStrategy {}

impl std::hash::Hash for AllForOneStrategy {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.max_nr_of_retries.hash(state);
    self.within_duration.hash(state);
    self.decider.hash(state);
  }
}

#[cfg(test)]
mod tests;
