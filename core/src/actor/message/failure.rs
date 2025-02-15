use crate::actor::ErrorReason;
use crate::actor::ExtendedPid;
use crate::actor::RestartStatistics;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::Message;
use nexus_actor_message_derive_rs::Message;
use std::any::Any;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
  pub who: ExtendedPid,
  pub reason: ErrorReason,
  pub restart_stats: RestartStatistics,
  pub message_handle: MessageHandle,
}

impl Message for Failure {
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other) = other.as_any().downcast_ref::<Self>() {
      self == other
    } else {
      false
    }
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn message_type(&self) -> &'static str {
    "Failure"
  }
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
