use crate::actor::actor_inner_error::ErrorReason;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ActorError {
  #[error("Receive error: {0}")]
  ReceiveError(ErrorReason),
  #[error("Restart error: {0}")]
  RestartError(ErrorReason),
  #[error("Stop error: {0}")]
  StopError(ErrorReason),
  #[error("Initialization error: {0}")]
  InitializationError(ErrorReason),
  #[error("Communication error: {0}")]
  CommunicationError(ErrorReason),
  #[error("Behavior not initialized: {0}")]
  BehaviorNotInitialized(ErrorReason),
}

impl ActorError {
  pub fn reason(&self) -> Option<&ErrorReason> {
    match self {
      ActorError::ReceiveError(e)
      | ActorError::RestartError(e)
      | ActorError::StopError(e)
      | ActorError::InitializationError(e)
      | ActorError::CommunicationError(e)
      | ActorError::BehaviorNotInitialized(e) => Some(e),
    }
  }
}

static_assertions::assert_impl_all!(ActorError: Send, Sync);
