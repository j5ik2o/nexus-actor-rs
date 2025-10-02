use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rand::Rng;

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::ErrorReason;
use crate::actor::core::RestartStatistics;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::supervisor_strategy::{log_failure, Supervisor, SupervisorHandle, SupervisorStrategy};
use nexus_actor_core_rs::actor::core_types::pid::CorePid;

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

#[async_trait]
impl SupervisorStrategy for ExponentialBackoffStrategy {
  async fn handle_child_failure(
    &self,
    actor_system: ActorSystem,
    supervisor: SupervisorHandle,
    child: CorePid,
    mut rs: RestartStatistics,
    reason: ErrorReason,
    _: MessageHandle,
  ) {
    self.set_failure_count(&mut rs).await;

    let failure_count = rs.failure_count().await as u64;
    let base = self.initial_backoff.map(|v| v.as_nanos()).unwrap_or_default();
    let backoff = failure_count.saturating_mul(base as u64);
    let noise = rand::rng().random_range(0..500);
    let dur = Duration::from_nanos(backoff + noise);

    let scheduler = actor_system.core_runtime().scheduler();
    let actor_system_clone = actor_system.clone();
    let supervisor_clone = supervisor.clone();
    let child_clone = child.clone();
    let reason_clone = reason.clone();

    scheduler.schedule_once(
      dur,
      Arc::new(move || {
        let actor_system = actor_system_clone.clone();
        let supervisor = supervisor_clone.clone();
        let child = child_clone.clone();
        let reason = reason_clone.clone();
        Box::pin(async move {
          log_failure(actor_system.clone(), &child, reason.clone(), Directive::Restart).await;
          supervisor.restart_children(&[child]).await;
        })
      }),
    );
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[cfg(test)]
mod tests;
