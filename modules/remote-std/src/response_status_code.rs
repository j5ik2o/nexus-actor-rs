use crate::generated::remote::ActorPidResponse;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(i32)]
pub enum ResponseStatusCode {
  Ok = 0,
  Unavailable,
  Timeout,
  ProcessNameAlreadyExists,
  Error,
  DeadLetter,
  Max,
}

impl ResponseStatusCode {
  pub const fn to_i32(self) -> i32 {
    self as i32
  }

  pub fn from_i32(value: i32) -> Option<Self> {
    Self::try_from(value).ok()
  }

  pub fn ensure_ok(self) -> Result<(), ResponseError> {
    match self.as_error() {
      None => Ok(()),
      Some(err) => Err(err),
    }
  }

  pub fn as_error(self) -> Option<ResponseError> {
    match self {
      ResponseStatusCode::Ok => None,
      ResponseStatusCode::Unavailable => Some(ResponseError::UNAVAILABLE),
      ResponseStatusCode::Timeout => Some(ResponseError::TIMEOUT),
      ResponseStatusCode::ProcessNameAlreadyExists => Some(ResponseError::PROCESS_NAME_ALREADY_EXISTS),
      ResponseStatusCode::Error => Some(ResponseError::UNKNOWN),
      ResponseStatusCode::DeadLetter => Some(ResponseError::DEAD_LETTER),
      ResponseStatusCode::Max => Some(ResponseError::UNKNOWN),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResponseError {
  code: ResponseStatusCode,
}

impl ResponseError {
  pub const DEAD_LETTER: Self = Self::new(ResponseStatusCode::DeadLetter);
  pub const PROCESS_NAME_ALREADY_EXISTS: Self = Self::new(ResponseStatusCode::ProcessNameAlreadyExists);
  pub const TIMEOUT: Self = Self::new(ResponseStatusCode::Timeout);
  pub const UNAVAILABLE: Self = Self::new(ResponseStatusCode::Unavailable);
  pub const UNKNOWN: Self = Self::new(ResponseStatusCode::Error);

  pub const fn new(code: ResponseStatusCode) -> Self {
    Self { code }
  }

  pub const fn code(self) -> ResponseStatusCode {
    self.code
  }

  pub fn is_retryable(self) -> bool {
    matches!(self.code, ResponseStatusCode::Unavailable | ResponseStatusCode::Timeout)
  }
}

impl fmt::Display for ResponseError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self.code)
  }
}

impl std::error::Error for ResponseError {}

pub trait ActorPidResponseExt {
  fn status_code_enum(&self) -> ResponseStatusCode;
  #[cfg_attr(not(test), allow(dead_code))]
  fn status_error(&self) -> Option<ResponseError>;
}

impl ActorPidResponseExt for ActorPidResponse {
  fn status_code_enum(&self) -> ResponseStatusCode {
    ResponseStatusCode::from_i32(self.status_code).unwrap_or(ResponseStatusCode::Error)
  }

  fn status_error(&self) -> Option<ResponseError> {
    self.status_code_enum().as_error()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ensure_ok_accepts_ok() {
    assert!(ResponseStatusCode::Ok.ensure_ok().is_ok());
  }

  #[test]
  fn ensure_ok_converts_error() {
    let err = ResponseStatusCode::Timeout.ensure_ok().unwrap_err();
    assert_eq!(err.code(), ResponseStatusCode::Timeout);
    assert!(err.is_retryable());
  }

  #[test]
  fn actor_pid_response_ext_maps_status() {
    let response = ActorPidResponse {
      pid: None,
      status_code: ResponseStatusCode::DeadLetter.to_i32(),
    };
    assert_eq!(response.status_code_enum(), ResponseStatusCode::DeadLetter);
    assert_eq!(response.status_error(), Some(ResponseError::DEAD_LETTER));
  }
}
