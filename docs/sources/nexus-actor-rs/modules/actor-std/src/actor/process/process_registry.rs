use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::actor::actor_system::{ActorSystem, WeakActorSystem};
use crate::actor::core::ActorProcess;
use crate::actor::core::ExtendedPid;
use crate::actor::process::{Process, ProcessHandle, ProcessMaps};
use crate::generated::actor::Pid;
use dashmap::mapref::entry::Entry;
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::actor::core_types::process_registry::{
  CoreAddressResolver, CoreProcessRegistry, CoreProcessRegistryFuture,
};
use std::string::String as StdString;

#[cfg(test)]
mod tests;

const LOCAL_ADDRESS: &str = "nonhost";

#[derive(Debug, Clone)]
pub struct ProcessRegistry {
  sequence_id: Arc<AtomicU64>,
  actor_system: WeakActorSystem,
  address: Arc<RwLock<String>>,
  local_pids: Arc<ProcessMaps>,
  remote_handlers: Arc<RwLock<Vec<AddressResolver>>>,
}

pub type AddressResolver = CoreAddressResolver<ProcessHandle>;

impl ProcessRegistry {
  pub fn new(actor_system: ActorSystem) -> Self {
    Self {
      sequence_id: Arc::new(AtomicU64::new(0)),
      actor_system: actor_system.downgrade(),
      address: Arc::new(RwLock::new(LOCAL_ADDRESS.to_string())),
      local_pids: Arc::new(ProcessMaps::new()),
      remote_handlers: Arc::new(RwLock::new(Vec::new())),
    }
  }

  fn actor_system(&self) -> ActorSystem {
    self
      .actor_system
      .upgrade()
      .expect("ActorSystem dropped before ProcessRegistry")
  }

  pub fn register_address_resolver(&self, handler: AddressResolver) {
    let mut mg = self.remote_handlers.write().unwrap();
    mg.push(handler);
  }

  pub fn set_address(&self, address: String) {
    let mut mg = self.address.write().unwrap();
    *mg = address;
  }

  pub fn get_address(&self) -> String {
    self.address.read().unwrap().clone()
  }

  pub async fn list_local_pids(&self) -> Vec<Pid> {
    let address = self.get_address();
    self
      .local_pids
      .keys()
      .into_iter()
      .map(|id| Pid {
        address: address.clone(),
        id,
        request_id: 0,
      })
      .collect()
  }

  pub async fn find_local_process_handle(&self, id: &str) -> Option<ProcessHandle> {
    self.local_pids.get_if_present(id)
  }

  pub fn next_id(&self) -> String {
    let counter = self.sequence_id.fetch_add(1, Ordering::SeqCst);
    uint64_to_id(counter)
  }

  pub async fn add_process(&self, process: ProcessHandle, id: &str) -> (ExtendedPid, bool) {
    let process_map = self.local_pids.get_map(id);
    let pid = Pid {
      address: self.get_address(),
      id: id.to_string(),
      request_id: 0,
    };
    let pid = ExtendedPid::new(pid);
    let inserted = match process_map.entry(id.to_string()) {
      Entry::Occupied(_) => false,
      Entry::Vacant(vacant) => {
        vacant.insert(process);
        true
      }
    };
    (pid, inserted)
  }

  pub async fn remove_process(&self, pid: &ExtendedPid) {
    let process_map = self.local_pids.get_map(pid.id());
    if let Some((_, process)) = process_map.remove(pid.id()) {
      if let Some(actor_process) = process.as_any().downcast_ref::<ActorProcess>() {
        actor_process.set_dead();
      }
    }
  }

  pub async fn get_process(&self, pid: &ExtendedPid) -> Option<ProcessHandle> {
    let is_remote = pid.address() != LOCAL_ADDRESS && pid.address() != self.get_address();
    if is_remote {
      let handlers = { self.remote_handlers.read().unwrap().clone() };
      for handler in handlers {
        if let Some(process) = handler.run(pid.core_pid()).await {
          return Some(process);
        }
      }
      return Some(self.actor_system().get_dead_letter().await);
    }
    self.get_local_process(pid.id()).await
  }

  pub async fn get_local_process(&self, id: &str) -> Option<ProcessHandle> {
    let process_map = self.local_pids.get_map(id);
    match process_map.get(id) {
      Some(r) => Some(r.clone()),
      None => Some(self.actor_system().get_dead_letter().await),
    }
  }
}

impl CoreProcessRegistry for ProcessRegistry {
  type Handle = ProcessHandle;

  fn register_address_resolver(&self, resolver: AddressResolver) {
    ProcessRegistry::register_address_resolver(self, resolver);
  }

  fn set_address(&self, address: StdString) {
    ProcessRegistry::set_address(self, address);
  }

  fn get_address(&self) -> StdString {
    ProcessRegistry::get_address(self)
  }

  fn next_id(&self) -> StdString {
    ProcessRegistry::next_id(self)
  }

  fn list_local_pids<'a>(&'a self) -> CoreProcessRegistryFuture<'a, Vec<CorePid>> {
    let registry = self.clone();
    Box::pin(async move {
      let address = registry.get_address();
      registry
        .local_pids
        .keys()
        .into_iter()
        .map(|id| CorePid::new(address.clone(), id).with_request_id(0))
        .collect()
    })
  }

  fn find_local_process_handle<'a>(&'a self, id: &'a str) -> CoreProcessRegistryFuture<'a, Option<Self::Handle>> {
    let registry = self.clone();
    let id = id.to_string();
    Box::pin(async move { ProcessRegistry::find_local_process_handle(&registry, &id).await })
  }

  fn add_process<'a>(&'a self, process: Self::Handle, id: &'a str) -> CoreProcessRegistryFuture<'a, (CorePid, bool)> {
    let registry = self.clone();
    let id = id.to_string();
    Box::pin(async move {
      let (extended, inserted) = ProcessRegistry::add_process(&registry, process, &id).await;
      (extended.to_core(), inserted)
    })
  }

  fn remove_process<'a>(&'a self, pid: &'a CorePid) -> CoreProcessRegistryFuture<'a, ()> {
    let registry = self.clone();
    let pid = pid.clone();
    Box::pin(async move {
      let extended = ExtendedPid::from_core(pid);
      ProcessRegistry::remove_process(&registry, &extended).await;
    })
  }

  fn get_process<'a>(&'a self, pid: &'a CorePid) -> CoreProcessRegistryFuture<'a, Option<Self::Handle>> {
    let registry = self.clone();
    let pid = pid.clone();
    Box::pin(async move {
      let extended = ExtendedPid::from_core(pid);
      ProcessRegistry::get_process(&registry, &extended).await
    })
  }

  fn get_local_process<'a>(&'a self, id: &'a str) -> CoreProcessRegistryFuture<'a, Option<Self::Handle>> {
    let registry = self.clone();
    let id = id.to_string();
    Box::pin(async move { ProcessRegistry::get_local_process(&registry, &id).await })
  }
}

const DIGITS: &[u8; 64] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ~+";

pub fn uint64_to_id(u: u64) -> String {
  // 最大13文字 (62進数で11桁 + '$' + 潜在的な追加の1文字)
  let mut buf = [0u8; 13];
  let mut i = buf.len() - 1;
  let mut u = u;

  while u >= 64 {
    buf[i] = DIGITS[(u & 0x3f) as usize];
    u >>= 6;
    i -= 1;
  }
  buf[i] = DIGITS[u as usize];
  i -= 1;
  buf[i] = b'$';

  // 使用された部分のスライスを文字列に変換
  unsafe { std::str::from_utf8_unchecked(&buf[i..]).to_string() }
}
