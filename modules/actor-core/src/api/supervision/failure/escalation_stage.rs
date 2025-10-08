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
