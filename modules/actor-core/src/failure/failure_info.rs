use crate::ActorId;
use crate::ActorPath;

use super::{EscalationStage, FailureMetadata};

/// 障害情報。protoactor-go の Failure メッセージを簡略化した形で保持する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureInfo {
  pub actor: ActorId,
  pub path: ActorPath,
  pub reason: alloc::string::String,
  pub metadata: FailureMetadata,
  pub stage: EscalationStage,
}

impl FailureInfo {
  pub fn new(actor: ActorId, path: ActorPath, reason: alloc::string::String) -> Self {
    Self::new_with_metadata(actor, path, reason, FailureMetadata::default())
  }

  pub fn new_with_metadata(
    actor: ActorId,
    path: ActorPath,
    reason: alloc::string::String,
    metadata: FailureMetadata,
  ) -> Self {
    Self {
      actor,
      path,
      reason,
      metadata,
      stage: EscalationStage::Initial,
    }
  }

  pub fn with_metadata(mut self, metadata: FailureMetadata) -> Self {
    self.metadata = metadata;
    self
  }

  pub fn with_stage(mut self, stage: EscalationStage) -> Self {
    self.stage = stage;
    self
  }

  pub fn from_error(actor: ActorId, path: ActorPath, error: &dyn core::fmt::Debug) -> Self {
    Self::from_error_with_metadata(actor, path, error, FailureMetadata::default())
  }

  pub fn from_error_with_metadata(
    actor: ActorId,
    path: ActorPath,
    error: &dyn core::fmt::Debug,
    metadata: FailureMetadata,
  ) -> Self {
    Self {
      actor,
      path,
      reason: alloc::format!("{:?}", error),
      metadata,
      stage: EscalationStage::Initial,
    }
  }

  pub fn escalate_to_parent(&self) -> Option<Self> {
    let parent_path = self.path.parent()?;
    let parent_actor = parent_path.last().unwrap_or(self.actor);
    Some(Self {
      actor: parent_actor,
      path: parent_path,
      reason: self.reason.clone(),
      metadata: self.metadata.clone(),
      stage: self.stage.escalate(),
    })
  }
}
