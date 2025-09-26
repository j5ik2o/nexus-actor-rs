use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::ErrorReason;
use crate::actor::core::ExtendedPid;
use crate::actor::core::RestartStatistics;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::supervisor_strategy::{
  log_failure, record_supervisor_metrics, Decider, Supervisor, SupervisorHandle, SupervisorStrategy,
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
    Fut: futures::future::Future<Output = Directive> + Send + 'static,
  {
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

#[async_trait]
impl SupervisorStrategy for OneForOneStrategy {
  async fn handle_child_failure(
    &self,
    actor_system: ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    mut rs: RestartStatistics,
    reason: ErrorReason,
    message_handle: MessageHandle,
  ) {
    tracing::debug!(
      "OneForOneStrategy::handle_child_failure: child = {:?}, rs = {:?}, message = {:?}",
      child.id(),
      rs,
      message_handle
    );
    let child_pid = child.id().to_string();
    let record_decision = |decision: &str| {
      record_supervisor_metrics(&supervisor, "one_for_one", decision, &child_pid, Vec::new());
    };

    let directive = self.decider.run(reason.clone()).await;
    match directive {
      Directive::Resume => {
        record_decision("resume");
        // resume the failing child
        tracing::debug!(
          "OneForOneStrategy::handle_child_failure: Resume: child = {:?}, rs = {:?}, message = {:?}",
          child.id(),
          rs,
          message_handle
        );
        log_failure(actor_system, &child, reason, directive).await;
        supervisor.resume_children(&[child]).await
      }
      Directive::Restart => {
        tracing::debug!(
          "OneForOneStrategy::handle_child_failure: Restart: child = {:?}, rs = {:?}, message = {:?}",
          child.id(),
          rs,
          message_handle
        );
        // try restart the failing child
        if self.should_stop(&mut rs).await {
          record_decision("stop_after_restart_attempt");
          log_failure(actor_system, &child, reason, Directive::Stop).await;
          supervisor.stop_children(&[child]).await;
        } else {
          record_decision("restart");
          log_failure(actor_system, &child, reason, Directive::Restart).await;
          supervisor.restart_children(&[child]).await;
        }
      }
      Directive::Stop => {
        record_decision("stop");
        tracing::debug!(
          "OneForOneStrategy::handle_child_failure: Stop: child = {:?}, rs = {:?}, message = {:?}",
          child.id(),
          rs,
          message_handle
        );
        // stop the failing child, no need to involve the crs
        log_failure(actor_system, &child, reason, directive).await;
        supervisor.stop_children(&[child]).await
      }
      Directive::Escalate => {
        record_decision("escalate");
        tracing::debug!(
          "OneForOneStrategy::handle_child_failure: Escalate: child = {:?}, rs = {:?}, message = {:?}",
          child.id(),
          rs,
          message_handle
        );
        // send failure to parent
        // supervisor mailbox
        // do not log here, log in the parent handling the error
        supervisor.escalate_failure(reason, message_handle).await
      }
    }
  }

  fn as_any(&self) -> &dyn Any {
    self
  }
}

#[cfg(test)]
mod tests;
