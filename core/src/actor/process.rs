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

pub type ActorProcess = Box<dyn Process + Send + Sync>;
pub type ProcessHandle = Box<dyn Process + Send + Sync>;

// Helper functions for creating process handles
impl ProcessHandle {
    pub fn new(process: Box<dyn Process + Send + Sync>) -> Self {
        process
    }
}
