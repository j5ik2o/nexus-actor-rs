use alloc::vec::Vec;
use core::fmt;

use crate::ActorId;

/// Hierarchical path of an actor.
///
/// Represents actor location in a hierarchical structure, holding the path from root as a sequence of segments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActorPath {
  segments: Vec<ActorId>,
}

impl ActorPath {
  /// Creates a new empty `ActorPath`.
  ///
  /// Represents a root path with no segments.
  pub fn new() -> Self {
    Self { segments: Vec::new() }
  }

  /// Gets the sequence of path segments.
  ///
  /// # Returns
  ///
  /// Slice of `ActorId`
  pub fn segments(&self) -> &[ActorId] {
    &self.segments
  }

  /// Creates a new path with a child actor ID added.
  ///
  /// # Arguments
  ///
  /// * `id` - ID of the child actor to add
  ///
  /// # Returns
  ///
  /// New `ActorPath` including the child actor
  pub fn push_child(&self, id: ActorId) -> Self {
    let mut segments = self.segments.clone();
    segments.push(id);
    Self { segments }
  }

  /// Gets the parent actor's path.
  ///
  /// # Returns
  ///
  /// `Some(ActorPath)` if parent path exists, `None` for root
  pub fn parent(&self) -> Option<Self> {
    if self.segments.is_empty() {
      None
    } else {
      let mut segments = self.segments.clone();
      segments.pop();
      Some(Self { segments })
    }
  }

  /// Gets the last segment (actor ID) of the path.
  ///
  /// # Returns
  ///
  /// Last `ActorId`, or `None` if empty
  pub fn last(&self) -> Option<ActorId> {
    self.segments.last().copied()
  }

  /// Checks if the path is empty (root).
  ///
  /// # Returns
  ///
  /// `true` if empty, `false` otherwise
  pub fn is_empty(&self) -> bool {
    self.segments.is_empty()
  }
}

impl Default for ActorPath {
  fn default() -> Self {
    Self::new()
  }
}

impl fmt::Display for ActorPath {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self.segments.is_empty() {
      return f.write_str("/");
    }

    for segment in &self.segments {
      write!(f, "/{}", segment)?;
    }
    Ok(())
  }
}
