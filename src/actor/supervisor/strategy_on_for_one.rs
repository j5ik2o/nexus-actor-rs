use async_trait::async_trait;

use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::actor::ActorInnerError;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::supervisor_strategy::{
  log_failure, DeciderFunc, Supervisor, SupervisorHandle, SupervisorStrategy,
};

#[derive(Debug, Clone)]
pub struct OneForOneStrategy {
  max_retries: u32,
  within_duration: tokio::time::Duration,
  decider: DeciderFunc,
}

impl OneForOneStrategy {
  pub fn new(max_retries: u32, within_duration: tokio::time::Duration, decider: DeciderFunc) -> Self {
    OneForOneStrategy {
      max_retries,
      within_duration,
      decider,
    }
  }

  async fn should_stop(&self, rs: &mut RestartStatistics) -> bool {
    if self.max_retries == 0 {
      true
    } else {
      rs.fail().await;
      if rs.number_of_failures(self.within_duration).await > self.max_retries {
        rs.reset().await;
        true
      } else {
        false
      }
    }
  }
}

impl PartialEq for OneForOneStrategy {
  fn eq(&self, other: &Self) -> bool {
    self.max_retries == other.max_retries
      && self.within_duration == other.within_duration
      && self.decider == other.decider
  }
}

impl Eq for OneForOneStrategy {}

impl std::hash::Hash for OneForOneStrategy {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.max_retries.hash(state);
    self.within_duration.hash(state);
    self.decider.hash(state);
  }
}

#[async_trait]
impl SupervisorStrategy for OneForOneStrategy {
  async fn handle_failure(
    &self,
    actor_system: &ActorSystem,
    mut supervisor: SupervisorHandle,
    child: ExtendedPid,
    mut rs: RestartStatistics,
    reason: ActorInnerError,
    message: MessageHandle,
  ) {
    tracing::debug!(
      "OneForOneStrategy::handle_failure: child = {:?}, rs = {:?}, message = {:?}",
      child,
      rs,
      message
    );
    let directive = self.decider.run(reason.clone());
    match directive {
      Directive::Resume => {
        // resume the failing child
        tracing::debug!(
          "OneForOneStrategy::handle_failure: Resume: child = {:?}, rs = {:?}, message = {:?}",
          child,
          rs,
          message
        );
        log_failure(actor_system, &child, reason, directive).await;
        supervisor.resume_children(&[child]).await
      }
      Directive::Restart => {
        tracing::debug!(
          "OneForOneStrategy::handle_failure: Restart: child = {:?}, rs = {:?}, message = {:?}",
          child,
          rs,
          message
        );
        // try restart the failing child
        if self.should_stop(&mut rs).await {
          log_failure(actor_system, &child, reason, Directive::Stop).await;
          supervisor.stop_children(&[child]).await;
        } else {
          log_failure(actor_system, &child, reason, Directive::Restart).await;
          supervisor.restart_children(&[child]).await;
        }
      }
      Directive::Stop => {
        tracing::debug!(
          "OneForOneStrategy::handle_failure: Stop: child = {:?}, rs = {:?}, message = {:?}",
          child,
          rs,
          message
        );
        // stop the failing child, no need to involve the crs
        log_failure(actor_system, &child, reason, directive).await;
        supervisor.stop_children(&[child]).await
      }
      Directive::Escalate => {
        tracing::debug!(
          "OneForOneStrategy::handle_failure: Escalate: child = {:?}, rs = {:?}, message = {:?}",
          child,
          rs,
          message
        );
        // send failure to parent
        // supervisor mailbox
        // do not log here, log in the parent handling the error
        supervisor.escalate_failure(reason, message).await
      }
    }
  }
}
