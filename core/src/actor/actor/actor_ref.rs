use crate::actor::message::Message;
use crate::actor::pid::Pid;
use async_trait::async_trait;
use std::fmt::Debug;

#[async_trait]
pub trait ActorRef: Debug + Send + Sync {
    async fn send(&self, message: Box<dyn Message>);
    async fn stop(&self);
    fn pid(&self) -> Pid;
}

#[derive(Debug, Clone)]
pub struct LocalActorRef {
    pid: Pid,
}

impl LocalActorRef {
    pub fn new(pid: Pid) -> Self {
        Self { pid }
    }
}

#[async_trait]
impl ActorRef for LocalActorRef {
    async fn send(&self, message: Box<dyn Message>) {
        // Implementation will be added later
    }

    async fn stop(&self) {
        // Implementation will be added later
    }

    fn pid(&self) -> Pid {
        self.pid.clone()
    }
}
