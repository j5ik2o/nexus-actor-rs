use std::fmt::Display;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::actor::actor::actor_process::ActorProcess;
use crate::actor::actor::Pid;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::process::{Process, ProcessHandle};

impl Pid {
  pub(crate) fn new(address: &str, id: &str) -> Self {
    Pid {
      address: address.to_string(),
      id: id.to_string(),
      request_id: 0,
    }
  }

  pub(crate) fn with_request_id(mut self, request_id: u32) -> Self {
    self.request_id = request_id;
    self
  }
}

impl std::fmt::Display for Pid {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}-{}-{}", self.address, self.id, self.request_id)
  }
}

#[derive(Debug, Clone)]
pub struct ExtendedPid {
  pub inner: Pid,
  actor_system: ActorSystem,
  process_handle: Arc<Mutex<Option<ProcessHandle>>>,
}

impl Display for ExtendedPid {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.inner)
  }
}

impl PartialEq for ExtendedPid {
  fn eq(&self, other: &Self) -> bool {
    self.inner == other.inner
  }
}

impl Eq for ExtendedPid {}

unsafe impl Send for ExtendedPid {}
unsafe impl Sync for ExtendedPid {}

impl ExtendedPid {
  pub fn new(pid: Pid, actor_system: ActorSystem) -> Self {
    Self {
      inner: pid,
      actor_system,
      process_handle: Arc::new(Mutex::new(None)),
    }
  }

  pub fn address(&self) -> &str {
    &self.inner.address
  }

  pub fn id(&self) -> &str {
    &self.inner.id
  }

  pub fn request_id(&self) -> u32 {
    self.inner.request_id
  }

  pub(crate) async fn ref_process(&self, actor_system: ActorSystem) -> ProcessHandle {
    let mut process_handle_opt = self.process_handle.lock().await;
    if let Some(process) = process_handle_opt.as_ref() {
      if let Some(actor_process) = process.as_any().downcast_ref::<ActorProcess>() {
        if actor_process.is_dead() {
          *process_handle_opt = None;
        } else {
          return process.clone();
        }
      } else {
        return process.clone();
      }
    }

    let process_registry = actor_system.get_process_registry().await;
    if let Some(process_handle) = process_registry.get_process(self).await {
      *process_handle_opt = Some(process_handle.clone());
      process_handle
    } else {
      panic!("No process found for pid: {}", self)
    }
  }

  pub async fn send_user_message(&self, actor_system: ActorSystem, message: MessageHandle) {
    self
      .ref_process(actor_system)
      .await
      .send_user_message(Some(self), message)
      .await;
  }

  pub async fn send_system_message(&self, actor_system: ActorSystem, message: MessageHandle) {
    self
      .ref_process(actor_system)
      .await
      .send_system_message(&self, message)
      .await;
  }
}
