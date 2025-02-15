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

pub trait PidExt {
    fn to_extended(&self) -> ExtendedPid;
    fn to_pid(&self) -> Pid;
    fn to_generated(&self) -> GeneratedPid;
}

impl PidExt for Pid {
    fn to_extended(&self) -> ExtendedPid {
        ExtendedPid {
            address: self.address.clone(),
            id: self.id,
        }
    }

    fn to_pid(&self) -> Pid {
        self.clone()
    }

    fn to_generated(&self) -> GeneratedPid {
        GeneratedPid {
            address: self.address.clone(),
            id: self.id,
        }
    }
}

impl PidExt for ExtendedPid {
    fn to_extended(&self) -> ExtendedPid {
        self.clone()
    }

    fn to_pid(&self) -> Pid {
        Pid {
            address: self.address.clone(),
            id: self.id,
        }
    }

    fn to_generated(&self) -> GeneratedPid {
        GeneratedPid {
            address: self.address.clone(),
            id: self.id,
        }
    }
}

impl PidExt for GeneratedPid {
    fn to_extended(&self) -> ExtendedPid {
        ExtendedPid {
            address: self.address.clone(),
            id: self.id,
        }
    }

    fn to_pid(&self) -> Pid {
        Pid {
            address: self.address.clone(),
            id: self.id,
        }
    }

    fn to_generated(&self) -> GeneratedPid {
        self.clone()
    }
}

impl ExtendedPid {
    pub fn new(pid: impl PidExt) -> Self {
        pid.to_extended()
    }

    pub async fn send_user_message(&self, process: &impl Process, message: impl Into<Box<dyn Message>>) {
        process.send_user_message(Some(&self.to_pid()), message.into()).await;
    }

    pub async fn send_system_message(&self, process: &impl Process, message: impl Into<Box<dyn Message>>) {
        process.send_system_message(message.into()).await;
    }

    pub async fn stop(&self, process: &impl Process) {
        process.stop().await;
    }
}

impl Pid {
    pub fn new(address: String, id: u64) -> Self {
        Self { address, id }
    }

    pub async fn send_user_message(&self, process: &impl Process, message: impl Into<Box<dyn Message>>) {
        process.send_user_message(Some(self), message.into()).await;
    }

    pub async fn send_system_message(&self, process: &impl Process, message: impl Into<Box<dyn Message>>) {
        process.send_system_message(message.into()).await;
    }

    pub async fn stop(&self, process: &impl Process) {
        process.stop().await;
    }
}
