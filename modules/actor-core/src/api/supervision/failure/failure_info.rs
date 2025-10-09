use crate::ActorId;
use crate::ActorPath;

use super::{EscalationStage, FailureMetadata};

/// Failure information. Holds a simplified form of protoactor-go's Failure message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureInfo {
  /// ID of the actor where the failure occurred
  pub actor: ActorId,
  /// Path of the actor where the failure occurred
  pub path: ActorPath,
  /// String describing the reason for the failure
  pub reason: alloc::string::String,
  /// Metadata associated with the failure
  pub metadata: FailureMetadata,
  /// Escalation stage
  pub stage: EscalationStage,
}

impl FailureInfo {
  /// Creates new failure information with default metadata.
  ///
  /// # Arguments
  /// * `actor` - ID of the actor where the failure occurred
  /// * `path` - Path of the actor where the failure occurred
  /// * `reason` - String describing the reason for the failure
  ///
  /// # Returns
  /// New `FailureInfo` instance
  pub fn new(actor: ActorId, path: ActorPath, reason: alloc::string::String) -> Self {
    Self::new_with_metadata(actor, path, reason, FailureMetadata::default())
  }

  /// Creates new failure information with specified metadata.
  ///
  /// # Arguments
  /// * `actor` - ID of the actor where the failure occurred
  /// * `path` - Path of the actor where the failure occurred
  /// * `reason` - String describing the reason for the failure
  /// * `metadata` - Metadata associated with the failure
  ///
  /// # Returns
  /// New `FailureInfo` instance
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

  /// Sets metadata.
  ///
  /// # Arguments
  /// * `metadata` - Metadata to set
  ///
  /// # Returns
  /// `FailureInfo` instance with metadata set
  pub fn with_metadata(mut self, metadata: FailureMetadata) -> Self {
    self.metadata = metadata;
    self
  }

  /// Sets escalation stage.
  ///
  /// # Arguments
  /// * `stage` - Escalation stage to set
  ///
  /// # Returns
  /// `FailureInfo` instance with escalation stage set
  pub fn with_stage(mut self, stage: EscalationStage) -> Self {
    self.stage = stage;
    self
  }

  /// Creates failure information from an error with default metadata.
  ///
  /// # Arguments
  /// * `actor` - ID of the actor where the failure occurred
  /// * `path` - Path of the actor where the failure occurred
  /// * `error` - Error object
  ///
  /// # Returns
  /// New `FailureInfo` instance
  pub fn from_error(actor: ActorId, path: ActorPath, error: &dyn core::fmt::Debug) -> Self {
    Self::from_error_with_metadata(actor, path, error, FailureMetadata::default())
  }

  /// Creates failure information from an error and metadata.
  ///
  /// # Arguments
  /// * `actor` - ID of the actor where the failure occurred
  /// * `path` - Path of the actor where the failure occurred
  /// * `error` - Error object
  /// * `metadata` - Metadata associated with the failure
  ///
  /// # Returns
  /// New `FailureInfo` instance
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

  /// Creates new failure information escalated to parent actor.
  ///
  /// # Returns
  /// `FailureInfo` instance escalated to parent actor.
  /// Returns `None` if parent doesn't exist.
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
