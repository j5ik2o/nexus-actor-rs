use std::any::Any;
use std::sync::Arc;

use crate::actor::actor::actor_inner_error::ActorInnerError;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::supervisor_strategy::{
  log_failure, DeciderFunc, Supervisor, SupervisorHandle, SupervisorStrategy,
};
use async_trait::async_trait;

pub fn default_decider(_: ActorInnerError) -> Directive {
  Directive::Restart
}

#[derive(Debug, Clone)]
pub struct OneForOneStrategy {
  max_nr_of_retries: u32,
  pub(crate) within_duration: tokio::time::Duration,
  decider: Arc<DeciderFunc>,
}

impl OneForOneStrategy {
  pub fn new(max_nr_of_retries: u32, within_duration: tokio::time::Duration) -> Self {
    OneForOneStrategy {
      max_nr_of_retries,
      within_duration,
      decider: Arc::new(DeciderFunc::new(default_decider)),
    }
  }

  pub fn with_decider(mut self, decider: impl Fn(ActorInnerError) -> Directive + Send + Sync + 'static) -> Self {
    self.decider = Arc::new(DeciderFunc::new(decider));
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
    actor_system: &ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    mut rs: RestartStatistics,
    reason: ActorInnerError,
    message: MessageHandle,
  ) {
    tracing::debug!(
      "OneForOneStrategy::handle_child_failure: child = {:?}, rs = {:?}, message = {:?}",
      child.id(),
      rs,
      message
    );
    let directive = self.decider.run(reason.clone());
    match directive {
      Directive::Resume => {
        // resume the failing child
        tracing::debug!(
          "OneForOneStrategy::handle_child_failure: Resume: child = {:?}, rs = {:?}, message = {:?}",
          child.id(),
          rs,
          message
        );
        log_failure(actor_system, &child, reason, directive).await;
        supervisor.resume_children(&[child]).await
      }
      Directive::Restart => {
        tracing::debug!(
          "OneForOneStrategy::handle_child_failure: Restart: child = {:?}, rs = {:?}, message = {:?}",
          child.id(),
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
          "OneForOneStrategy::handle_child_failure: Stop: child = {:?}, rs = {:?}, message = {:?}",
          child.id(),
          rs,
          message
        );
        // stop the failing child, no need to involve the crs
        log_failure(actor_system, &child, reason, directive).await;
        supervisor.stop_children(&[child]).await
      }
      Directive::Escalate => {
        tracing::debug!(
          "OneForOneStrategy::handle_child_failure: Escalate: child = {:?}, rs = {:?}, message = {:?}",
          child.id(),
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

  fn as_any(&self) -> &dyn Any {
    self
  }
}
