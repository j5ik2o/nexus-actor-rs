use std::fmt::Display;
use std::hash::Hash;
use std::sync::Arc;

use crate::actor::actor_system::ActorSystem;
use crate::actor::core::actor_process::ActorProcess;
use crate::actor::message::MessageHandle;
use crate::actor::process::{Process, ProcessHandle};
use crate::generated::actor::Pid;

use nexus_actor_core_rs::actor::core_types::pid::{CorePid, CorePidRef};
use tokio::sync::Mutex;

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

  pub fn to_core(&self) -> CorePid {
    CorePid::new(self.address.clone(), self.id.clone()).with_request_id(self.request_id)
  }

  pub fn from_core(core: CorePid) -> Self {
    let (address, id, request_id) = core.into_parts();
    Pid {
      address,
      id,
      request_id,
    }
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
  core_pid: CorePid,
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
      core_pid: pid.to_core(),
      inner_pid: pid,
      process_handle: Arc::new(Mutex::new(None)),
    }
  }

  /// `CorePid` のスライスから `ExtendedPid` のベクタへ変換するヘルパー。
  pub fn from_core_slice(pids: &[CorePid]) -> Vec<Self> {
    pids.iter().map(Self::from).collect()
  }

  /// `CorePid` のイテレータから `ExtendedPid` のベクタへ変換するヘルパー。
  pub fn from_core_iter<I>(pids: I) -> Vec<Self>
  where
    I: IntoIterator<Item = CorePid>, {
    pids.into_iter().map(Self::from).collect()
  }

  /// `ExtendedPid` のスライスから `CorePid` のベクタへ変換するヘルパー。
  pub fn to_core_vec(pids: &[Self]) -> Vec<CorePid> {
    pids.iter().map(CorePid::from).collect()
  }

  pub fn address(&self) -> &str {
    self.core_pid.address()
  }

  pub fn id(&self) -> &str {
    self.core_pid.id()
  }

  pub fn request_id(&self) -> u32 {
    self.core_pid.request_id()
  }

  pub fn core_pid(&self) -> &CorePid {
    &self.core_pid
  }

  pub fn to_core(&self) -> CorePid {
    self.core_pid.clone()
  }

  pub fn to_pid(&self) -> Pid {
    self.inner_pid.clone()
  }

  pub fn from_core(core_pid: CorePid) -> Self {
    let (address, id, request_id) = core_pid.clone().into_parts();
    let proto_pid = Pid {
      address,
      id,
      request_id,
    };
    Self {
      core_pid,
      inner_pid: proto_pid,
      process_handle: Arc::new(Mutex::new(None)),
    }
  }

  pub(crate) async fn ref_process(&self, actor_system: ActorSystem) -> ProcessHandle {
    if let Some(handle) = self.take_live_cached_handle().await {
      return handle;
    }

    let process_registry = actor_system.get_process_registry().await;
    if let Some(process_handle) = process_registry.get_process(self).await {
      self.store_process_handle(process_handle).await
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

impl ExtendedPid {
  async fn take_live_cached_handle(&self) -> Option<ProcessHandle> {
    let mut guard = self.process_handle.lock().await;
    if let Some(handle) = guard.as_ref() {
      if Self::is_process_dead(handle) {
        *guard = None;
        None
      } else {
        Some(handle.clone())
      }
    } else {
      None
    }
  }

  async fn store_process_handle(&self, handle: ProcessHandle) -> ProcessHandle {
    let mut guard = self.process_handle.lock().await;
    if let Some(existing) = guard.as_ref() {
      if !Self::is_process_dead(existing) {
        return existing.clone();
      }
    }
    *guard = Some(handle.clone());
    handle
  }

  fn is_process_dead(process: &ProcessHandle) -> bool {
    process
      .as_any()
      .downcast_ref::<ActorProcess>()
      .map(|actor_process| actor_process.is_dead())
      .unwrap_or(false)
  }
}

impl From<CorePid> for ExtendedPid {
  fn from(core_pid: CorePid) -> Self {
    Self::from_core(core_pid)
  }
}

impl From<&CorePid> for ExtendedPid {
  fn from(core_pid: &CorePid) -> Self {
    Self::from_core(core_pid.clone())
  }
}

impl From<ExtendedPid> for CorePid {
  fn from(pid: ExtendedPid) -> Self {
    pid.core_pid
  }
}

impl From<&ExtendedPid> for CorePid {
  fn from(pid: &ExtendedPid) -> Self {
    pid.core_pid.clone()
  }
}

impl CorePidRef for ExtendedPid {
  fn as_core_pid(&self) -> &CorePid {
    &self.core_pid
  }
}
