use crate::actor::actor::ErrorReason;
use crate::actor::actor::ExtendedPid;
use crate::actor::actor::RestartStatistics;
use crate::actor::message::message::Message;
use crate::actor::message::message_handle::MessageHandle;
use nexus_actor_message_derive_rs::Message;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct Failure {
  pub who: ExtendedPid,
  pub reason: ErrorReason,
  pub restart_stats: RestartStatistics,
  pub message_handle: MessageHandle,
}

impl Failure {
  pub fn new(
    who: ExtendedPid,
    reason: ErrorReason,
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
