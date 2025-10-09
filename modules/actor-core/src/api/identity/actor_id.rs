use core::fmt;

/// アクターの一意な識別子。
///
/// 各アクターに割り当てられる数値IDで、アクターシステム内でアクターを一意に識別します。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorId(pub usize);

impl ActorId {
  /// ルートアクターを示す特別なID。
  ///
  /// `usize::MAX` の値を持ち、アクター階層のルートを表します。
  pub const ROOT: ActorId = ActorId(usize::MAX);
}

impl fmt::Display for ActorId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}
