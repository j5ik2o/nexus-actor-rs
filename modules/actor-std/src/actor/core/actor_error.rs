use crate::actor::core::error_reason::ErrorReason;
use nexus_actor_core_rs::actor::core_types::actor_error::CoreActorError;
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

  pub fn into_core(self) -> CoreActorError {
    match self {
      ActorError::ReceiveError(reason) => CoreActorError::receive(reason.into_core()),
      ActorError::RestartError(reason) => CoreActorError::restart(reason.into_core()),
      ActorError::StopError(reason) => CoreActorError::stop(reason.into_core()),
      ActorError::InitializationError(reason) => CoreActorError::initialization(reason.into_core()),
      ActorError::CommunicationError(reason) => CoreActorError::communication(reason.into_core()),
      ActorError::BehaviorNotInitialized(reason) => CoreActorError::behavior_not_initialized(reason.into_core()),
    }
  }

  pub fn from_core(core: CoreActorError) -> Self {
    match core {
      CoreActorError::Receive(reason) => Self::ReceiveError(ErrorReason::from_core(reason)),
      CoreActorError::Restart(reason) => Self::RestartError(ErrorReason::from_core(reason)),
      CoreActorError::Stop(reason) => Self::StopError(ErrorReason::from_core(reason)),
      CoreActorError::Initialization(reason) => Self::InitializationError(ErrorReason::from_core(reason)),
      CoreActorError::Communication(reason) => Self::CommunicationError(ErrorReason::from_core(reason)),
      CoreActorError::BehaviorNotInitialized(reason) => Self::BehaviorNotInitialized(ErrorReason::from_core(reason)),
    }
  }
}

impl From<CoreActorError> for ActorError {
  fn from(value: CoreActorError) -> Self {
    Self::from_core(value)
  }
}

impl From<ActorError> for CoreActorError {
  fn from(value: ActorError) -> Self {
    value.into_core()
  }
}

static_assertions::assert_impl_all!(ActorError: Send, Sync);

#[cfg(test)]
mod tests {
  use super::*;
  use std::string::String;

  #[test]
  fn roundtrip_core_conversion_preserves_variant() {
    let std_error = ActorError::of_restart_error(ErrorReason::from("restart"));
    let core_error = std_error.clone().into_core();
    match &core_error {
      CoreActorError::Restart(reason) => assert!(reason.is_type::<String>()),
      _ => panic!("unexpected variant"),
    }
    let restored = ActorError::from_core(core_error);
    assert_eq!(restored.reason().unwrap().is_type::<String>(), true);
  }
}
