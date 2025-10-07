#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorId(pub usize);

impl ActorId {
  pub const ROOT: ActorId = ActorId(usize::MAX);
}
