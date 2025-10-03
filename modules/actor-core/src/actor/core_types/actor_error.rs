#![cfg(feature = "alloc")]

use core::fmt;

use crate::error::ErrorReasonCore;

/// コアレイヤで共有する ActorError 相当の列挙。
/// no_std/alloc 環境でも `ErrorReasonCore` を利用してエラー内容を保持できる。
#[derive(Clone, PartialEq, Eq)]
pub enum CoreActorError {
  Receive(ErrorReasonCore),
  Restart(ErrorReasonCore),
  Stop(ErrorReasonCore),
  Initialization(ErrorReasonCore),
  Communication(ErrorReasonCore),
  BehaviorNotInitialized(ErrorReasonCore),
}

impl CoreActorError {
  #[must_use]
  pub fn receive(reason: ErrorReasonCore) -> Self {
    Self::Receive(reason)
  }

  #[must_use]
  pub fn restart(reason: ErrorReasonCore) -> Self {
    Self::Restart(reason)
  }

  #[must_use]
  pub fn stop(reason: ErrorReasonCore) -> Self {
    Self::Stop(reason)
  }

  #[must_use]
  pub fn initialization(reason: ErrorReasonCore) -> Self {
    Self::Initialization(reason)
  }

  #[must_use]
  pub fn communication(reason: ErrorReasonCore) -> Self {
    Self::Communication(reason)
  }

  #[must_use]
  pub fn behavior_not_initialized(reason: ErrorReasonCore) -> Self {
    Self::BehaviorNotInitialized(reason)
  }

  #[must_use]
  pub fn reason(&self) -> &ErrorReasonCore {
    match self {
      Self::Receive(reason)
      | Self::Restart(reason)
      | Self::Stop(reason)
      | Self::Initialization(reason)
      | Self::Communication(reason)
      | Self::BehaviorNotInitialized(reason) => reason,
    }
  }

  #[must_use]
  pub fn map_reason(self, f: impl FnOnce(ErrorReasonCore) -> ErrorReasonCore) -> Self {
    match self {
      Self::Receive(reason) => Self::Receive(f(reason)),
      Self::Restart(reason) => Self::Restart(f(reason)),
      Self::Stop(reason) => Self::Stop(f(reason)),
      Self::Initialization(reason) => Self::Initialization(f(reason)),
      Self::Communication(reason) => Self::Communication(f(reason)),
      Self::BehaviorNotInitialized(reason) => Self::BehaviorNotInitialized(f(reason)),
    }
  }
}

impl fmt::Debug for CoreActorError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("CoreActorError")
      .field(
        "kind",
        &match self {
          Self::Receive(_) => "Receive",
          Self::Restart(_) => "Restart",
          Self::Stop(_) => "Stop",
          Self::Initialization(_) => "Initialization",
          Self::Communication(_) => "Communication",
          Self::BehaviorNotInitialized(_) => "BehaviorNotInitialized",
        },
      )
      .field("reason", self.reason())
      .finish()
  }
}

impl fmt::Display for CoreActorError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.reason())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use alloc::format;
  use alloc::string::String;

  #[test]
  fn map_reason_replaces_inner_reason() {
    let original = CoreActorError::receive(ErrorReasonCore::from("before"));
    let mapped = original.clone().map_reason(|_| ErrorReasonCore::from("after"));
    assert_eq!(mapped.reason().code, 0);
    assert!(mapped.reason().is_type::<String>());
    let display = format!("{}", mapped);
    assert!(display.contains("type_id"));
    // 元の値は変更されない
    assert_eq!(original.reason().code, 0);
  }
}
