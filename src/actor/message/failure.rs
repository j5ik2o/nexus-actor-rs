use crate::actor::actor::actor_inner_error::ActorInnerError;
use crate::actor::actor::pid::ExtendedPid;
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::message::message::Message;
use crate::actor::message::message_handle::MessageHandle;
use std::any::Any;

#[derive(Debug, Clone)]
pub struct Failure {
  pub who: ExtendedPid,
  pub reason: ActorInnerError,
  pub restart_stats: RestartStatistics,
  pub message_handle: MessageHandle,
}

impl Message for Failure {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().is::<Failure>()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

impl Failure {
  pub fn new(
    who: ExtendedPid,
    reason: ActorInnerError,
    restart_stats: RestartStatistics,
    message_handle: MessageHandle,
  ) -> Self {
    Failure {
      who,
      reason,
      restart_stats,
      message_handle,
    }
  }
}
