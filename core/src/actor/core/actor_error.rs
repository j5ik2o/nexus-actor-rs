use crate::actor::core::error_reason::ErrorReason;
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

  pub fn of_receive_error(error_reason: ErrorReason) -> Self {
    ActorError::ReceiveError(error_reason)
  }

  pub fn of_restart_error(error_reason: ErrorReason) -> Self {
    ActorError::RestartError(error_reason)
  }

  pub fn of_stop_error(error_reason: ErrorReason) -> Self {
    ActorError::StopError(error_reason)
  }

  pub fn of_initialization_error(error_reason: ErrorReason) -> Self {
    ActorError::InitializationError(error_reason)
  }

  pub fn of_communication_error(error_reason: ErrorReason) -> Self {
    ActorError::CommunicationError(error_reason)
  }

  pub fn of_behavior_not_initialized(error_reason: ErrorReason) -> Self {
    ActorError::BehaviorNotInitialized(error_reason)
  }

  pub fn reason(&self) -> Option<&ErrorReason> {
    match self {
      ActorError::ReceiveError(e, ..)
      | ActorError::RestartError(e, ..)
      | ActorError::StopError(e, ..)
      | ActorError::InitializationError(e, ..)
      | ActorError::CommunicationError(e, ..)
      | ActorError::BehaviorNotInitialized(e, ..) => Some(e),
    }
  }
}

static_assertions::assert_impl_all!(ActorError: Send, Sync);
