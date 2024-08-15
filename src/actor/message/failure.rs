use nexus_acto_message_derive_rs::Message;
use crate::actor::actor::ActorInnerError;
use crate::actor::actor::ExtendedPid;
use crate::actor::actor::RestartStatistics;
use crate::actor::message::message::Message;
use crate::actor::message::message_handle::MessageHandle;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct Failure {
  pub who: ExtendedPid,
  pub reason: ActorInnerError,
  pub restart_stats: RestartStatistics,
  pub message_handle: MessageHandle,
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
