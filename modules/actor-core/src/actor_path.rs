use alloc::vec::Vec;

use crate::actor_id::ActorId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorPath {
  segments: Vec<ActorId>,
}

impl ActorPath {
  pub fn new() -> Self {
    Self { segments: Vec::new() }
  }

  pub fn segments(&self) -> &[ActorId] {
    &self.segments
  }

  pub fn push_child(&self, id: ActorId) -> Self {
    let mut segments = self.segments.clone();
    segments.push(id);
    Self { segments }
  }

  pub fn parent(&self) -> Option<Self> {
    if self.segments.is_empty() {
      None
    } else {
      let mut segments = self.segments.clone();
      segments.pop();
      Some(Self { segments })
    }
  }

  pub fn last(&self) -> Option<ActorId> {
    self.segments.last().copied()
  }

  pub fn is_empty(&self) -> bool {
    self.segments.is_empty()
  }
}

impl Default for ActorPath {
  fn default() -> Self {
    Self::new()
  }
}
