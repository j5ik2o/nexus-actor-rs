use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;

use crate::actor::core::ExtendedPid;
use crate::actor::message::MessageHandle;

use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::actor::core_types::process::{CoreProcessHandle, ProcessFuture};

pub mod actor_future;
mod dead_letter;
pub(crate) mod dead_letter_process;
pub mod future;
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

impl CoreProcessHandle for ProcessHandle {
  fn send_user_message<'a>(&'a self, pid: Option<&'a CorePid>, message: MessageHandle) -> ProcessFuture<'a> {
    let process = self.clone();
    let core_pid_opt = pid.cloned();
    Box::pin(async move {
      let extended_pid = core_pid_opt.as_ref().map(|p| ExtendedPid::from_core(p.clone()));
      Process::send_user_message(&process, extended_pid.as_ref(), message).await;
    })
  }

  fn send_system_message<'a>(&'a self, pid: &'a CorePid, message: MessageHandle) -> ProcessFuture<'a> {
    let process = self.clone();
    let core_pid = pid.clone();
    Box::pin(async move {
      let extended = ExtendedPid::from_core(core_pid);
      Process::send_system_message(&process, &extended, message).await;
    })
  }

  fn stop<'a>(&'a self, pid: &'a CorePid) -> ProcessFuture<'a> {
    let process = self.clone();
    let core_pid = pid.clone();
    Box::pin(async move {
      let extended = ExtendedPid::from_core(core_pid);
      Process::stop(&process, &extended).await;
    })
  }

  fn set_dead(&self) {
    Process::set_dead(self);
  }

  fn as_any(&self) -> &dyn std::any::Any {
    Process::as_any(self)
  }
}
