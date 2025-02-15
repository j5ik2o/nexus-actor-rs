use std::fmt::Debug;
use async_trait::async_trait;
use crate::actor::message::Message;
use crate::actor::pid::Pid;

#[async_trait]
pub trait Process: Debug + Send + Sync + 'static {
    async fn send_user_message(&self, sender: Option<&Pid>, message: Box<dyn Message>);
    async fn send_system_message(&self, message: Box<dyn Message>);
    async fn stop(&self);
    async fn set_dead(&self);
}

#[async_trait]
impl Process for crate::actor::dispatch::future::ActorFutureProcess {
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

#[async_trait]
impl Process for crate::actor::event_stream::event_stream_process::EventStreamProcess {
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

#[async_trait]
impl Process for crate::actor::guardian::GuardianProcess {
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
