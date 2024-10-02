use std::fmt::Display;
use std::hash::Hash;
use std::sync::Arc;

use crate::actor::actor::actor_process::ActorProcess;
use crate::actor::actor_system::ActorSystem;
use crate::actor::message::MessageHandle;
use crate::actor::process::{Process, ProcessHandle};
use crate::generated::actor::Pid;

use regex::Regex;
use tokio::sync::Mutex;

fn is_valid_address(input: &str) -> bool {
  let re = Regex::new(r"^((\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})|([a-zA-Z0-9\-\.]+)):\d+$").unwrap();
  re.is_match(input)
}

impl Pid {
  pub fn new(address: &str, id: &str) -> Self {
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

impl Hash for Pid {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.address.hash(state);
    self.id.hash(state);
    self.request_id.hash(state);
  }
}

#[derive(Debug, Clone)]
pub struct ExtendedPid {
  pub inner_pid: Pid,
  process_handle: Arc<Mutex<Option<ProcessHandle>>>,
}

impl Hash for ExtendedPid {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.inner_pid.hash(state);
  }
}

impl Display for ExtendedPid {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.inner_pid)
  }
}

impl PartialEq for ExtendedPid {
  fn eq(&self, other: &Self) -> bool {
    self.inner_pid == other.inner_pid
  }
}

impl Eq for ExtendedPid {}

static_assertions::assert_impl_all!(ExtendedPid: Send, Sync);

impl ExtendedPid {
  pub fn new(pid: Pid) -> Self {
    Self {
      inner_pid: pid,
      process_handle: Arc::new(Mutex::new(None)),
    }
  }

  pub fn address(&self) -> &str {
    &self.inner_pid.address
  }

  pub fn id(&self) -> &str {
    &self.inner_pid.id
  }

  pub fn request_id(&self) -> u32 {
    self.inner_pid.request_id
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

  pub async fn send_user_message(&self, actor_system: ActorSystem, message_handle: MessageHandle) {
    tracing::debug!("Sending user message to pid: {}", self);
    self
      .ref_process(actor_system)
      .await
      .send_user_message(Some(self), message_handle)
      .await;
  }

  pub async fn send_system_message(&self, actor_system: ActorSystem, message_handle: MessageHandle) {
    self
      .ref_process(actor_system)
      .await
      .send_system_message(self, message_handle)
      .await;
  }
}
