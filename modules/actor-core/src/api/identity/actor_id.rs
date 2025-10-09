use core::fmt;

/// Unique identifier for an actor.
///
/// Numeric ID assigned to each actor, uniquely identifying actors within the actor system.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorId(pub usize);

impl ActorId {
  /// Special ID indicating the root actor.
  ///
  /// Has the value of `usize::MAX` and represents the root of the actor hierarchy.
  pub const ROOT: ActorId = ActorId(usize::MAX);
}

impl fmt::Display for ActorId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}
