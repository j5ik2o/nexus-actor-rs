/// Escalation stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EscalationStage {
  /// Initial failure point.
  #[default]
  Initial,
  /// Propagating towards parent.
  Escalated {
    /// Number of propagations
    hops: u8,
  },
}

impl EscalationStage {
  /// Returns the initial stage.
  ///
  /// # Returns
  /// `EscalationStage::Initial` instance
  pub const fn initial() -> Self {
    EscalationStage::Initial
  }

  /// Returns the number of escalation propagations.
  ///
  /// # Returns
  /// Number of propagations. Returns 0 for `Initial`.
  pub fn hops(self) -> u8 {
    match self {
      EscalationStage::Initial => 0,
      EscalationStage::Escalated { hops } => hops,
    }
  }

  /// Checks if this is the initial stage.
  ///
  /// # Returns
  /// `true` if initial stage, `false` otherwise
  pub const fn is_initial(self) -> bool {
    matches!(self, EscalationStage::Initial)
  }

  /// Returns the next escalation stage.
  ///
  /// # Returns
  /// New escalated `EscalationStage` instance.
  /// Returns `Escalated { hops: 1 }` for `Initial`,
  /// or a new instance with `hops` incremented by 1 for `Escalated`.
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
