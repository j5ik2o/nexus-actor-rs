use crate::actor::core_types::{ActorRef, ActorRefError, BasicPid};
use crate::actor::message::MessageHandle;
use crate::generated::actor::Pid;
use async_trait::async_trait;
use std::fmt::{Debug, Display, Formatter};

/// Wrapper to convert between BasicPid and generated Pid
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PidWrapper {
  inner: BasicPid,
}

impl PidWrapper {
  pub fn new(address: &str, id: &str) -> Self {
    PidWrapper {
      inner: BasicPid::new(address, id),
    }
  }

  pub fn from_basic(basic: BasicPid) -> Self {
    PidWrapper { inner: basic }
  }

  pub fn from_generated(pid: &Pid) -> Self {
    PidWrapper {
      inner: BasicPid::new(&pid.address, &pid.id).with_request_id(pid.request_id),
    }
  }

  pub fn to_generated(&self) -> Pid {
    Pid {
      address: self.inner.address.clone(),
      id: self.inner.id.clone(),
      request_id: self.inner.request_id,
    }
  }

  pub fn to_basic(&self) -> BasicPid {
    self.inner.clone()
  }
}

impl Display for PidWrapper {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.inner)
  }
}

/// Simple ActorRef implementation for PidWrapper
pub struct SimplePidRef {
  pid: PidWrapper,
  // In a real implementation, this would hold a reference to the actor system
  // or process registry to actually send messages
}

impl SimplePidRef {
  pub fn new(pid: PidWrapper) -> Self {
    SimplePidRef { pid }
  }
}

impl Debug for SimplePidRef {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SimplePidRef({})", self.pid)
  }
}

#[async_trait]
impl ActorRef for SimplePidRef {
  fn get_id(&self) -> String {
    self.pid.inner.id.clone()
  }

  fn get_address(&self) -> String {
    self.pid.inner.address.clone()
  }

  async fn tell(&self, _message: MessageHandle) {
    // In a real implementation, this would use the process registry
    // to find the actor and send the message
    todo!("Implement message sending through process registry")
  }

  async fn request(
    &self,
    _message: MessageHandle,
    _timeout: std::time::Duration,
  ) -> Result<MessageHandle, ActorRefError> {
    // In a real implementation, this would create a future and wait for response
    todo!("Implement request-response through process registry")
  }

  fn is_alive(&self) -> bool {
    // In a real implementation, this would check the process registry
    true
  }
}
