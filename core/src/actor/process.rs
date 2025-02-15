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

pub type ProcessHandle = Box<dyn Process + Send + Sync>;

pub fn new_process_handle<P: Process + Send + Sync + 'static>(process: P) -> ProcessHandle {
  Box::new(process)
}
