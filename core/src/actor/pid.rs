use std::fmt::{Debug, Display, Formatter};
use crate::actor::process::Process;
use crate::actor::message::Message;
use crate::generated::actor::Pid as GeneratedPid;

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtendedPid {
    pub address: String,
    pub id: u64,
}

impl Display for ExtendedPid {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.id, self.address)
    }
}

impl ExtendedPid {
    pub fn new(pid: Pid) -> Self {
        Self {
            address: pid.address,
            id: pid.id,
        }
    }

    pub async fn send_user_message(&self, process: &impl Process, message: Box<dyn Message>) {
        process.send_user_message(Some(&self.into()), message).await;
    }

    pub async fn send_system_message(&self, process: &impl Process, message: Box<dyn Message>) {
        process.send_system_message(message).await;
    }

    pub async fn stop(&self, process: &impl Process) {
        process.stop().await;
    }
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
