use alloc::string::String;

use crate::actor_id::ActorId;
use crate::actor_path::ActorPath;

/// 障害情報。protoactor-go の Failure メッセージを簡略化した形で保持する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureInfo {
  pub actor: ActorId,
  pub path: ActorPath,
  pub reason: String,
}

impl FailureInfo {
  pub fn new(actor: ActorId, path: ActorPath, reason: String) -> Self {
    Self { actor, path, reason }
  }

  pub fn from_error(actor: ActorId, path: ActorPath, error: &dyn core::fmt::Debug) -> Self {
    Self {
      actor,
      path,
      reason: alloc::format!("{:?}", error),
    }
  }

  pub fn escalate_to_parent(&self) -> Option<Self> {
    let parent_path = self.path.parent()?;
    let parent_actor = parent_path.last().unwrap_or(self.actor);
    Some(Self {
      actor: parent_actor,
      path: parent_path,
      reason: self.reason.clone(),
    })
  }
}
