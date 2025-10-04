#![cfg(feature = "alloc")]

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::actor::core_types::pid::CorePid;
use crate::runtime::AsyncRwLock;

pub struct CorePidSet<PL, LL>
where
  PL: AsyncRwLock<Vec<CorePid>> + Send + Sync + 'static,
  LL: AsyncRwLock<BTreeMap<String, CorePid>> + Send + Sync + 'static, {
  pids: Arc<PL>,
  lookup: Arc<LL>,
}

impl<PL, LL> CorePidSet<PL, LL>
where
  PL: AsyncRwLock<Vec<CorePid>> + Send + Sync + 'static,
  LL: AsyncRwLock<BTreeMap<String, CorePid>> + Send + Sync + 'static,
{
  pub fn new() -> Self
  where
    PL: Default,
    LL: Default, {
    Self {
      pids: Arc::new(PL::default()),
      lookup: Arc::new(LL::default()),
    }
  }

  pub fn from_locks(pids: Arc<PL>, lookup: Arc<LL>) -> Self {
    Self { pids, lookup }
  }

  pub fn pids_lock(&self) -> Arc<PL> {
    self.pids.clone()
  }

  pub fn lookup_lock(&self) -> Arc<LL> {
    self.lookup.clone()
  }

  fn key(pid: &CorePid) -> String {
    format!("{}::{}", pid.address(), pid.id())
  }

  pub async fn contains(&self, pid: &CorePid) -> bool {
    let key = Self::key(pid);
    self.lookup.read().await.contains_key(&key)
  }

  pub async fn add(&self, pid: CorePid) {
    if self.contains(&pid).await {
      return;
    }
    let key = Self::key(&pid);
    self.lookup.write().await.insert(key, pid.clone());
    self.pids.write().await.push(pid);
  }

  pub async fn remove(&self, pid: &CorePid) -> bool {
    let key = Self::key(pid);
    let mut lookup_guard = self.lookup.write().await;
    if lookup_guard.remove(&key).is_some() {
      let mut pids_guard = self.pids.write().await;
      if let Some(pos) = pids_guard.iter().position(|candidate| candidate == pid) {
        pids_guard.remove(pos);
      }
      true
    } else {
      false
    }
  }

  pub async fn len(&self) -> usize {
    self.pids.read().await.len()
  }

  pub async fn is_empty(&self) -> bool {
    self.len().await == 0
  }

  pub async fn clear(&self) {
    self.lookup.write().await.clear();
    self.pids.write().await.clear();
  }

  pub async fn to_vec(&self) -> Vec<CorePid> {
    self.pids.read().await.clone()
  }

  pub async fn get(&self, index: usize) -> Option<CorePid> {
    self.pids.read().await.get(index).cloned()
  }

  pub async fn for_each<F, Fut>(&self, mut f: F)
  where
    F: FnMut(usize, CorePid) -> Fut,
    Fut: core::future::Future<Output = ()> + Send, {
    let snapshot = self.pids.read().await.clone();
    for (idx, pid) in snapshot.into_iter().enumerate() {
      f(idx, pid).await;
    }
  }
}

impl<PL, LL> Default for CorePidSet<PL, LL>
where
  PL: AsyncRwLock<Vec<CorePid>> + Default + Send + Sync + 'static,
  LL: AsyncRwLock<BTreeMap<String, CorePid>> + Default + Send + Sync + 'static,
{
  fn default() -> Self {
    Self::new()
  }
}

impl<PL, LL> Clone for CorePidSet<PL, LL>
where
  PL: AsyncRwLock<Vec<CorePid>> + Send + Sync + 'static,
  LL: AsyncRwLock<BTreeMap<String, CorePid>> + Send + Sync + 'static,
{
  fn clone(&self) -> Self {
    Self {
      pids: Arc::clone(&self.pids),
      lookup: Arc::clone(&self.lookup),
    }
  }
}

impl<PL, LL> core::fmt::Debug for CorePidSet<PL, LL>
where
  PL: AsyncRwLock<Vec<CorePid>> + Send + Sync + 'static,
  LL: AsyncRwLock<BTreeMap<String, CorePid>> + Send + Sync + 'static,
{
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    f.debug_struct("CorePidSet")
      .field("pids_ptr", &Arc::as_ptr(&self.pids))
      .field("lookup_ptr", &Arc::as_ptr(&self.lookup))
      .finish()
  }
}
