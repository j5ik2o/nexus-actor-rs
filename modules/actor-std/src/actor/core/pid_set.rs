use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::TokioRwLock;
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::runtime::AsyncRwLock;

use crate::generated::actor::Pid;

#[derive(Debug)]
struct PidSetInner {
  pids: TokioRwLock<Vec<CorePid>>,
  lookup: TokioRwLock<HashMap<String, CorePid>>,
}

impl Default for PidSetInner {
  fn default() -> Self {
    Self {
      pids: TokioRwLock::new(Vec::new()),
      lookup: TokioRwLock::new(HashMap::new()),
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct PidSet {
  inner: Arc<PidSetInner>,
}

impl PidSet {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn new_with_pids(pids: &[Pid]) -> Self {
    let core_pids: Vec<CorePid> = pids.iter().map(Self::to_core_pid).collect();
    let lookup_map = core_pids
      .iter()
      .map(|pid| (Self::key(pid), pid.clone()))
      .collect::<HashMap<_, _>>();
    Self {
      inner: Arc::new(PidSetInner {
        pids: TokioRwLock::new(core_pids),
        lookup: TokioRwLock::new(lookup_map),
      }),
    }
  }

  fn key(pid: &CorePid) -> String {
    format!("{}::{}", pid.address(), pid.id())
  }

  pub async fn contains(&self, pid: &Pid) -> bool {
    let core_pid = Self::to_core_pid(pid);
    let lookup = self.inner.lookup.read().await;
    lookup.contains_key(&Self::key(&core_pid))
  }

  pub async fn add(&self, pid: Pid) {
    let core_pid = Self::to_core_pid(&pid);
    if self.contains(&pid).await {
      return;
    }
    let key = Self::key(&core_pid);
    self.inner.lookup.write().await.insert(key, core_pid.clone());
    self.inner.pids.write().await.push(core_pid);
  }

  pub async fn remove(&self, pid: &Pid) -> bool {
    let core_pid = Self::to_core_pid(pid);
    if let Some(index) = self.index_of(&core_pid).await {
      self.inner.lookup.write().await.remove(&Self::key(&core_pid));
      self.inner.pids.write().await.remove(index);
      true
    } else {
      false
    }
  }

  pub async fn len(&self) -> usize {
    self.inner.pids.read().await.len()
  }

  pub async fn is_empty(&self) -> bool {
    self.len().await == 0
  }

  pub async fn clear(&self) {
    self.inner.lookup.write().await.clear();
    self.inner.pids.write().await.clear();
  }

  pub async fn to_vec(&self) -> Vec<Pid> {
    self.inner.pids.read().await.iter().map(Self::to_proto_pid).collect()
  }

  pub async fn for_each<F, Fut>(&self, mut f: F)
  where
    F: FnMut(usize, Pid) -> Fut,
    Fut: core::future::Future<Output = ()> + Send,
  {
    let snapshot = self.inner.pids.read().await.clone();
    for (idx, pid) in snapshot.into_iter().enumerate() {
      f(idx, Self::to_proto_pid(&pid)).await;
    }
  }

  pub async fn get(&self, index: usize) -> Option<Pid> {
    self.inner.pids.read().await.get(index).map(Self::to_proto_pid)
  }

  async fn index_of(&self, pid: &CorePid) -> Option<usize> {
    let pids = self.inner.pids.read().await;
    pids.iter().position(|candidate| candidate == pid)
  }

  fn to_core_pid(pid: &Pid) -> CorePid {
    pid.to_core()
  }

  fn to_proto_pid(pid: &CorePid) -> Pid {
    Pid::from_core(pid.clone())
  }
}

#[cfg(test)]
mod tests;
