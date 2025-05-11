use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;

use crate::actor::core::ExtendedPid;
use crate::actor::message::MessageHandle;

mod process_maps;
pub mod process_registry;

use process_maps::*;

#[async_trait]
pub trait Process: Debug + Send + Sync + 'static {
  async fn send_user_message(&self, pid: Option<&ExtendedPid>, message_handle: MessageHandle);
  async fn send_system_message(&self, pid: &ExtendedPid, message_handle: MessageHandle);
  async fn stop(&self, pid: &ExtendedPid);

  fn set_dead(&self);

  // fn is_dead(&self) -> bool;
  fn as_any(&self) -> &dyn std::any::Any;
}

#[derive(Debug, Clone)]
pub struct ProcessHandle(Arc<dyn Process>);

impl PartialEq for ProcessHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for ProcessHandle {}

impl std::hash::Hash for ProcessHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Process).hash(state);
  }
}

impl ProcessHandle {
  pub fn new_arc(process: Arc<dyn Process>) -> Self {
    ProcessHandle(process)
  }

  pub fn new<P>(process: P) -> Self
  where
    P: Process + 'static, {
    ProcessHandle(Arc::new(process))
  }
}

#[async_trait]
impl Process for ProcessHandle {
  async fn send_user_message(&self, pid: Option<&ExtendedPid>, message_handle: MessageHandle) {
    tracing::debug!("ProcessHandle#send_user_message: {:?}", message_handle);
    self.0.send_user_message(pid, message_handle).await;
  }

  async fn send_system_message(&self, pid: &ExtendedPid, message_handle: MessageHandle) {
    self.0.send_system_message(pid, message_handle).await;
  }

  async fn stop(&self, pid: &ExtendedPid) {
    self.0.stop(pid).await;
  }

  fn set_dead(&self) {
    self.0.set_dead();
  }

  fn as_any(&self) -> &dyn std::any::Any {
    self.0.as_any()
  }
}
