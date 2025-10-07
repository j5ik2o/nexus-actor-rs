use alloc::string::String;

use crate::actor_id::ActorId;

/// 障害情報。protoactor-go の Failure メッセージを簡略化した形で保持する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureInfo {
  pub actor: ActorId,
  pub reason: String,
}

impl FailureInfo {
  pub fn new(actor: ActorId, reason: String) -> Self {
    Self { actor, reason }
  }

  pub fn from_error(actor: ActorId, error: &dyn core::fmt::Debug) -> Self {
    Self {
      actor,
      reason: alloc::format!("{:?}", error),
    }
  }
}
