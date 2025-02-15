use async_trait::async_trait;
use std::any::Any;
use std::sync::Arc;
use std::time::Duration;

use crate::actor::ErrorReason;
use crate::actor::ExtendedPid;
use crate::actor::RestartStatistics;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::MessageHandle;
use crate::actor::supervisor::directive::Directive;
use crate::actor::supervisor::strategy_one_for_one::default_decider;
use crate::actor::supervisor::supervisor_strategy::{
  log_failure, Decider, Supervisor, SupervisorHandle, SupervisorStrategy,
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

#[async_trait]
impl SupervisorStrategy for AllForOneStrategy {
  async fn handle_child_failure(
    &self,
    actor_system: ActorSystem,
    supervisor: SupervisorHandle,
    child: ExtendedPid,
    mut rs: RestartStatistics,
    reason: ErrorReason,
    message_handle: MessageHandle,
  ) {
    let directive = self.decider.run(reason.clone()).await;
    match directive {
      Directive::Resume => {
        log_failure(actor_system, &child, reason, directive).await;
        supervisor.resume_children(&[child]).await;
      }
      Directive::Restart => {
        let children = supervisor.get_children().await;
        if self.should_stop(&mut rs).await {
          log_failure(actor_system, &child, reason, Directive::Stop).await;
          supervisor.stop_children(&children).await;
        } else {
          log_failure(actor_system, &child, reason, Directive::Restart).await;
          supervisor.restart_children(&children).await;
        }
      }
      Directive::Stop => {
        let children = supervisor.get_children().await;
        log_failure(actor_system, &child, reason, directive).await;
        supervisor.stop_children(&children).await;
      }
      Directive::Escalate => {
        supervisor.escalate_failure(reason, message_handle).await;
      }
    }
  }

  fn as_any(&self) -> &dyn Any {
    self
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
