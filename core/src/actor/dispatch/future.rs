use std::fmt::Debug;
use async_trait::async_trait;
use crate::actor::message::{Message, MessageHandle};
use crate::actor::pid::Pid;
use crate::actor::process::Process;

#[derive(Debug)]
pub struct ActorFutureProcess {
    // Implementation details...
}

#[async_trait]
impl Process for ActorFutureProcess {
    async fn send_user_message(&self, sender: Option<&Pid>, message: Box<dyn Message>) {
        // Implementation
    }

    async fn send_system_message(&self, message: Box<dyn Message>) {
        // Implementation
    }

    async fn stop(&self) {
        // Implementation
    }

    async fn set_dead(&self) {
        // Implementation
    }
}
