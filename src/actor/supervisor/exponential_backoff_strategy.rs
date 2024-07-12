use std::any::Any;
use std::time::Duration;

use async_trait::async_trait;
use rand::Rng;

use crate::actor::actor::actor_inner_error::ActorInnerError;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::supervisor_strategy::{log_failure, Supervisor, SupervisorHandle, SupervisorStrategy};

#[derive(Debug)]
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
    actor_system: &ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    mut rs: RestartStatistics,
    reason: ActorInnerError,
    _: MessageHandle,
  ) {
    self.set_failure_count(&mut rs).await;

    let backoff = rs.failure_count().await as u64 * self.initial_backoff.map(|v| v.as_nanos()).unwrap_or(0) as u64;
    let noise = rand::thread_rng().gen_range(0..500);
    let dur = Duration::from_nanos(backoff + noise);

    let actor_system = actor_system.clone();
    let supervisor = supervisor.clone();
    let child = child.clone();
    let reason = reason.clone();

    tokio::spawn(async move {
      tokio::time::sleep(dur).await;
      log_failure(&actor_system, &child, reason.clone(), Directive::Restart).await;
      supervisor.restart_children(&[child]).await;
    });
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}
