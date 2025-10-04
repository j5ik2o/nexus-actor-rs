use crate::actor::core::ErrorReason;
use crate::actor::core::ExtendedPid;
use crate::actor::core::RestartStatistics;
use crate::actor::message::message_base::Message;
use crate::actor::message::message_handle::MessageHandle;
use nexus_message_derive_rs::Message;

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
