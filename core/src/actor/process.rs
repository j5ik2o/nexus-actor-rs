use crate::actor::message::Message;
use crate::actor::pid::Pid;
use async_trait::async_trait;
use std::fmt::Debug;

#[async_trait]
pub trait Process: Debug + Send + Sync + 'static {
  async fn send_user_message(&self, sender: Option<&Pid>, message: Box<dyn Message>);
  async fn send_system_message(&self, message: Box<dyn Message>);
  async fn stop(&self);
  async fn set_dead(&self);
}

pub type ActorProcess = Box<dyn Process + Send + Sync>;
pub type ProcessHandle = Box<dyn Process + Send + Sync>;

impl Clone for Box<dyn Process + Send + Sync> {
  fn clone(&self) -> Self {
    Box::new(ClonedProcess(self.as_ref()))
  }
}

#[derive(Debug)]
struct ClonedProcess<'a>(&'a (dyn Process + Send + Sync));

#[async_trait]
impl<'a> Process for ClonedProcess<'a> {
  async fn send_user_message(&self, sender: Option<&Pid>, message: Box<dyn Message>) {
    self.0.send_user_message(sender, message).await
  }

  async fn send_system_message(&self, message: Box<dyn Message>) {
    self.0.send_system_message(message).await
  }

  async fn stop(&self) {
    self.0.stop().await
  }

  async fn set_dead(&self) {
    self.0.set_dead().await
  }
}
