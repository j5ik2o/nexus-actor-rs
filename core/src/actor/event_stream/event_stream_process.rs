use std::fmt::Debug;
use async_trait::async_trait;
use crate::actor::message::{Message, MessageHandle};
use crate::actor::pid::{Pid, PidExt};
use crate::actor::process::Process;
use crate::actor::actor_system::ActorSystem;

#[derive(Debug, Clone)]
pub struct EventStreamProcess {
    actor_system: ActorSystem,
}

impl EventStreamProcess {
    pub fn new(actor_system: ActorSystem) -> Self {
        Self { actor_system }
    }
}

#[async_trait]
impl Process for EventStreamProcess {
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
