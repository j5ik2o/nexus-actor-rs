use std::fmt::{Debug, Display};
use std::hash::Hash;

/// Basic PID type that represents an actor's unique identifier
/// This is a simplified version without dependencies on other actor components
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicPid {
  pub address: String,
  pub id: String,
  pub request_id: u32,
}

impl BasicPid {
  pub fn new(address: &str, id: &str) -> Self {
    BasicPid {
      address: address.to_string(),
      id: id.to_string(),
      request_id: 0,
    }
  }

  pub fn with_request_id(mut self, request_id: u32) -> Self {
    self.request_id = request_id;
    self
  }
}

impl Display for BasicPid {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}-{}-{}", self.address, self.id, self.request_id)
  }
}

impl Hash for BasicPid {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.address.hash(state);
    self.id.hash(state);
    self.request_id.hash(state);
  }
}

/// System messages that don't depend on actor implementation
#[derive(Debug, Clone, PartialEq)]
pub enum SystemMessage {
  /// Watch another actor for termination
  Watch { watcher: BasicPid },
  /// Stop watching another actor
  Unwatch { watcher: BasicPid },
  /// Actor has terminated
  Terminated {
    who: BasicPid,
    reason: SystemTerminateReason,
  },
  /// Stop the actor
  Stop,
  /// Restart the actor
  Restart,
}

/// Termination info for system messages
#[derive(Debug, Clone, PartialEq)]
pub enum SystemTerminateReason {
  Normal,
  Failure(String),
  Shutdown,
}
