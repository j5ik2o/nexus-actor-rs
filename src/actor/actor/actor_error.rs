use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::actor::actor::actor_inner_error::ActorInnerError;

#[derive(Debug, Clone)]
pub enum ActorError {
  ReceiveError(ActorInnerError),
  RestartError(ActorInnerError),
  StopError(ActorInnerError),
  InitializationError(ActorInnerError),
  CommunicationError(ActorInnerError),
}

impl ActorError {
  pub fn reason(&self) -> Option<&ActorInnerError> {
    match self {
      ActorError::ReceiveError(e)
      | ActorError::RestartError(e)
      | ActorError::StopError(e)
      | ActorError::InitializationError(e)
      | ActorError::CommunicationError(e) => Some(e),
    }
  }
}

impl Display for ActorError {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    match self {
      ActorError::ReceiveError(e) => write!(f, "Receive error: {}", e),
      ActorError::RestartError(e) => write!(f, "Restart error: {}", e),
      ActorError::StopError(e) => write!(f, "Stop error: {}", e),
      ActorError::InitializationError(e) => write!(f, "Initialization error: {}", e),
      ActorError::CommunicationError(e) => write!(f, "Communication error: {}", e),
    }
  }
}

impl Error for ActorError {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self {
      ActorError::ReceiveError(e)
      | ActorError::RestartError(e)
      | ActorError::StopError(e)
      | ActorError::InitializationError(e)
      | ActorError::CommunicationError(e) => Some(e),
    }
  }
}
