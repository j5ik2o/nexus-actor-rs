use std::fmt::Debug;
use async_trait::async_trait;
use crate::actor::message::Message;
use crate::actor::pid::Pid;

pub type ErrorReason = Box<dyn std::error::Error + Send + Sync>;
pub type ActorProcess = Box<dyn Process + Send + Sync>;
pub type ActorProducer = Box<dyn Producer + Send + Sync>;
pub type ActorReceiver = Box<dyn Receiver + Send + Sync>;

#[async_trait]
pub trait Process: Debug + Send + Sync {
    async fn send_user_message(&self, sender: Option<&Pid>, message: Box<dyn Message>);
    async fn send_system_message(&self, message: Box<dyn Message>);
    async fn stop(&self);
}

#[async_trait]
pub trait Producer: Debug + Send + Sync {
    async fn produce(&self) -> ActorProcess;
}

#[async_trait]
pub trait Receiver: Debug + Send + Sync {
    async fn receive(&mut self, message: Box<dyn Message>);
}
