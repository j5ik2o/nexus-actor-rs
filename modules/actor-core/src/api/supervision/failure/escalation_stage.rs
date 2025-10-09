/// エスカレーションの段階。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EscalationStage {
  /// 最初の障害発生地点。
  #[default]
  Initial,
  /// 親方向へ伝播中。
  Escalated {
    /// 伝播回数
    hops: u8
  },
}

impl EscalationStage {
  /// 初期段階を返す。
  ///
  /// # Returns
  /// `EscalationStage::Initial`インスタンス
  pub const fn initial() -> Self {
    EscalationStage::Initial
  }

  /// エスカレーションの伝播回数を返す。
  ///
  /// # Returns
  /// 伝播回数。`Initial`の場合は0を返す。
  pub fn hops(self) -> u8 {
    match self {
      EscalationStage::Initial => 0,
      EscalationStage::Escalated { hops } => hops,
    }
  }

  /// 初期段階かどうかを判定する。
  ///
  /// # Returns
  /// 初期段階の場合は`true`、それ以外は`false`
  pub const fn is_initial(self) -> bool {
    matches!(self, EscalationStage::Initial)
  }

  /// 次のエスカレーション段階を返す。
  ///
  /// # Returns
  /// エスカレートされた新しい`EscalationStage`インスタンス。
  /// `Initial`の場合は`Escalated { hops: 1 }`、
  /// `Escalated`の場合は`hops`を1増やした新しいインスタンスを返す。
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
