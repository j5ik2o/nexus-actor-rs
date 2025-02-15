use crate::actor::process::Process;
use crate::actor::message::{Message, MessageHandle};
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
