use crate::actor::message::Message;
use crate::actor::process::Process;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pid {
  pub address: String,
  pub id: u64,
}

impl Display for Pid {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}@{}", self.id, self.address)
  }
}

impl From<ExtendedPid> for Pid {
  fn from(pid: ExtendedPid) -> Self {
    Self {
      address: pid.address,
      id: pid.id,
    }
  }
}

impl From<&ExtendedPid> for Pid {
  fn from(pid: &ExtendedPid) -> Self {
    Self {
      address: pid.address.clone(),
      id: pid.id,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtendedPid {
  pub address: String,
  pub id: u64,
}

impl From<Pid> for ExtendedPid {
  fn from(pid: Pid) -> Self {
    Self {
      address: pid.address,
      id: pid.id,
    }
  }
}

impl From<&Pid> for ExtendedPid {
  fn from(pid: &Pid) -> Self {
    Self {
      address: pid.address.clone(),
      id: pid.id,
    }
  }
}

impl ExtendedPid {
  pub fn new(pid: impl Into<ExtendedPid>) -> Self {
    pid.into()
  }
}

impl Pid {
  pub fn new(address: String, id: u64) -> Self {
    Self { address, id }
  }

  pub async fn send_user_message(&self, process: &impl Process, message: Box<dyn Message>) {
    process.send_user_message(Some(self), message).await;
  }

  pub async fn send_system_message(&self, process: &impl Process, message: Box<dyn Message>) {
    process.send_system_message(message).await;
  }

  pub async fn stop(&self, process: &impl Process) {
    process.stop().await;
  }
}
