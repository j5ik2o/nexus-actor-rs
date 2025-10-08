use alloc::collections::BTreeMap;
use alloc::string::String;

use crate::actor_id::ActorId;
use crate::actor_path::ActorPath;

/// Failure に付随するメタデータ。将来 remote/cluster 層の情報を保持するために使用する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureMetadata {
  pub component: Option<String>,
  pub endpoint: Option<String>,
  pub transport: Option<String>,
  pub tags: BTreeMap<String, String>,
}

impl FailureMetadata {
  pub fn new() -> Self {
    Self {
      component: None,
      endpoint: None,
      transport: None,
      tags: BTreeMap::new(),
    }
  }

  pub fn with_component(mut self, component: impl Into<String>) -> Self {
    self.component = Some(component.into());
    self
  }

  pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
    self.endpoint = Some(endpoint.into());
    self
  }

  pub fn with_transport(mut self, transport: impl Into<String>) -> Self {
    self.transport = Some(transport.into());
    self
  }

  pub fn insert_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
    self.tags.insert(key.into(), value.into());
    self
  }
}

impl Default for FailureMetadata {
  fn default() -> Self {
    Self::new()
  }
}

/// エスカレーションの段階。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EscalationStage {
  /// 最初の障害発生地点。
  #[default]
  Initial,
  /// 親方向へ伝播中。`hops` は伝播回数。
  Escalated { hops: u8 },
}

impl EscalationStage {
  pub const fn initial() -> Self {
    EscalationStage::Initial
  }

  pub fn hops(self) -> u8 {
    match self {
      EscalationStage::Initial => 0,
      EscalationStage::Escalated { hops } => hops,
    }
  }

  pub const fn is_initial(self) -> bool {
    matches!(self, EscalationStage::Initial)
  }

  pub fn escalate(self) -> Self {
    match self {
      EscalationStage::Initial => EscalationStage::Escalated { hops: 1 },
      EscalationStage::Escalated { hops } => {
        let next = hops.saturating_add(1);
        EscalationStage::Escalated { hops: next }
      }
    }
  }
}

/// 障害情報。protoactor-go の Failure メッセージを簡略化した形で保持する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureInfo {
  pub actor: ActorId,
  pub path: ActorPath,
  pub reason: String,
  pub metadata: FailureMetadata,
  pub stage: EscalationStage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailureEvent {
  RootEscalated(FailureInfo),
}

impl FailureInfo {
  pub fn new(actor: ActorId, path: ActorPath, reason: String) -> Self {
    Self::new_with_metadata(actor, path, reason, FailureMetadata::default())
  }

  pub fn new_with_metadata(actor: ActorId, path: ActorPath, reason: String, metadata: FailureMetadata) -> Self {
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor_id::ActorId;
  use crate::actor_path::ActorPath;

  #[test]
  fn escalation_stage_increments_with_parent_hops() {
    let root = ActorId(0);
    let child = ActorId(1);
    let grandchild = ActorId(2);

    let path = ActorPath::new()
      .push_child(root)
      .push_child(child)
      .push_child(grandchild);
    let failure = FailureInfo::new(grandchild, path, "boom".into());
    assert!(matches!(failure.stage, EscalationStage::Initial));

    let parent_failure = failure.escalate_to_parent().expect("parent exists");
    assert!(matches!(parent_failure.stage, EscalationStage::Escalated { hops: 1 }));

    let root_failure = parent_failure.escalate_to_parent().expect("root exists");
    assert!(matches!(root_failure.stage, EscalationStage::Escalated { hops: 2 }));
    assert_eq!(root_failure.path.segments(), &[root]);
  }
}
