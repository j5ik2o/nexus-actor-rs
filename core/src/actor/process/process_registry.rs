use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use futures::future::BoxFuture;
use siphasher::sip::SipHasher;
use tokio::sync::RwLock;

use crate::actor::actor::ActorProcess;
use crate::actor::actor::ExtendedPid;
use crate::actor::actor_system::ActorSystem;
use crate::actor::process::{Process, ProcessHandle};
use crate::generated::actor::Pid;

const LOCAL_ADDRESS: &str = "nonhost";

#[derive(Debug, Clone)]
pub struct ProcessRegistry {
  sequence_id: Arc<AtomicU64>,
  actor_system: ActorSystem,
  address: Arc<RwLock<String>>,
  local_pids: SliceMap,
  remote_handlers: Arc<RwLock<Vec<AddressResolver>>>,
}

#[derive(Debug, Clone)]
pub struct SliceMap {
  local_pids: Arc<Vec<DashMap<String, ProcessHandle>>>,
}

impl SliceMap {
  fn new() -> Self {
    let mut local_pids = Vec::with_capacity(1024);
    for _ in 0..1024 {
      local_pids.push(DashMap::new());
    }
    Self {
      local_pids: Arc::new(local_pids),
    }
  }

  fn get_bucket(&self, key: &str) -> &DashMap<String, ProcessHandle> {
    let mut hasher = SipHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();
    let index = (hash % 1024) as usize;
    &self.local_pids[index]
  }
}

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct AddressResolver(
  Arc<dyn Fn(&ExtendedPid) -> BoxFuture<'static, Option<ProcessHandle>> + Send + Sync + 'static>,
);

impl Debug for AddressResolver {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "AddressResolver")
  }
}

impl PartialEq for AddressResolver {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for AddressResolver {}

impl Hash for AddressResolver {
  fn hash<H: Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(&ExtendedPid) -> BoxFuture<'static, Option<ProcessHandle>>).hash(state);
  }
}

impl AddressResolver {
  pub fn new<F, Fut>(f: F) -> Self
  where
    F: Fn(&ExtendedPid) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Option<ProcessHandle>> + Send + 'static, {
    AddressResolver(Arc::new(move |p| Box::pin(f(p))))
  }

  pub async fn run(&self, pid: &ExtendedPid) -> Option<ProcessHandle> {
    (self.0)(pid).await
  }
}

impl ProcessRegistry {
  pub fn new(actor_system: ActorSystem) -> Self {
    ProcessRegistry {
      sequence_id: Arc::new(AtomicU64::new(0)),
      actor_system,
      address: Arc::new(RwLock::new(LOCAL_ADDRESS.to_string())),
      local_pids: SliceMap::new(),
      remote_handlers: Arc::new(RwLock::new(Vec::new())),
    }
  }

  pub async fn register_address_resolver(&mut self, handler: AddressResolver) {
    let mut mg = self.remote_handlers.write().await;
    mg.push(handler);
  }

  pub async fn set_address(&mut self, address: String) {
    let mut mg = self.address.write().await;
    *mg = address;
  }

  pub async fn get_address(&self) -> String {
    let mg = self.address.read().await;
    mg.clone()
  }

  pub fn next_id(&self) -> String {
    let counter = self.sequence_id.fetch_add(1, Ordering::SeqCst);
    uint64_to_id(counter)
  }

  pub async fn add_process(&self, process: ProcessHandle, id: &str) -> (ExtendedPid, bool) {
    let bucket = self.local_pids.get_bucket(id);
    let pid = Pid {
      address: self.get_address().await.clone(),
      id: id.to_string(),
      request_id: 0,
    };
    let pid = ExtendedPid::new(pid);
    let inserted = bucket.insert(id.to_string(), process).is_none();
    (pid, inserted)
  }

  pub fn remove_process(&self, pid: &ExtendedPid) {
    let bucket = self.local_pids.get_bucket(pid.id());
    if let Some((_, process)) = bucket.remove(pid.id()) {
      if let Some(actor_process) = process.as_any().downcast_ref::<ActorProcess>() {
        actor_process.set_dead();
      }
    }
  }

  pub async fn get_process(&self, pid: &ExtendedPid) -> Option<ProcessHandle> {
    let is_remote = pid.address() != LOCAL_ADDRESS && pid.address() != self.get_address().await;
    if is_remote {
      {
        let mg = self.remote_handlers.read().await;
        for handler in mg.iter() {
          if let Some(process) = handler.run(pid).await {
            return Some(process);
          }
        }
      }
      return Some(self.actor_system.get_dead_letter().await);
    }
    self.get_local_process(pid.id()).await
  }

  pub async fn get_local_process(&self, id: &str) -> Option<ProcessHandle> {
    let bucket = self.local_pids.get_bucket(id);
    let result = bucket.get(id);
    match result {
      Some(r) => Some(r.clone()),
      None => Some(self.actor_system.get_dead_letter().await),
    }
  }
}

pub(crate) fn uint64_to_id(u: u64) -> String {
  const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ~+";
  let mut buf = [0u8; 13];
  let mut i = 12;
  let mut u = u;
  while u >= 64 {
    buf[i] = DIGITS[(u & 0x3f) as usize];
    u >>= 6;
    i -= 1;
  }
  buf[i] = DIGITS[u as usize];
  i -= 1;
  buf[i] = b'$';
  String::from_utf8(buf[i..].to_vec()).unwrap()
}
