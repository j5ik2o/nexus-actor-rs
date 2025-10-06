use std::collections::BTreeMap;
use std::sync::Arc;

use crate::runtime::StdAsyncRwLock;
use nexus_actor_core_rs::actor::core_types::pid::CorePid;
use nexus_actor_core_rs::actor::core_types::pid_set::CorePidSet;

use crate::generated::actor::Pid;

#[derive(Debug)]
pub struct PidSet {
  inner: CorePidSet<StdAsyncRwLock<Vec<CorePid>>, StdAsyncRwLock<BTreeMap<String, CorePid>>>,
}

impl Default for PidSet {
  fn default() -> Self {
    Self {
      inner: CorePidSet::from_locks(
        Arc::new(StdAsyncRwLock::new(Vec::new())),
        Arc::new(StdAsyncRwLock::new(BTreeMap::new())),
      ),
    }
  }
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
      .collect::<BTreeMap<_, _>>();
    Self {
      inner: CorePidSet::from_locks(
        Arc::new(StdAsyncRwLock::new(core_pids)),
        Arc::new(StdAsyncRwLock::new(lookup_map)),
      ),
    }
  }

  fn key(pid: &CorePid) -> String {
    format!("{}::{}", pid.address(), pid.id())
  }

  fn to_core_pid(pid: &Pid) -> CorePid {
    pid.to_core()
  }

  fn to_proto_pid(pid: &CorePid) -> Pid {
    Pid::from_core(pid.clone())
  }

  pub async fn contains(&self, pid: &Pid) -> bool {
    let core_pid = Self::to_core_pid(pid);
    self.inner.contains(&core_pid).await
  }

  pub async fn add(&self, pid: Pid) {
    let core_pid = Self::to_core_pid(&pid);
    self.inner.add(core_pid).await;
  }

  pub async fn remove(&self, pid: &Pid) -> bool {
    let core_pid = Self::to_core_pid(pid);
    self.inner.remove(&core_pid).await
  }

  pub async fn len(&self) -> usize {
    self.inner.len().await
  }

  pub async fn is_empty(&self) -> bool {
    self.inner.is_empty().await
  }

  pub async fn clear(&self) {
    self.inner.clear().await;
  }

  pub async fn to_vec(&self) -> Vec<Pid> {
    self.inner.to_vec().await.iter().map(Self::to_proto_pid).collect()
  }

  pub async fn for_each<F, Fut>(&self, mut f: F)
  where
    F: FnMut(usize, Pid) -> Fut,
    Fut: core::future::Future<Output = ()> + Send, {
    let snapshot = self.inner.to_vec().await;
    for (idx, pid) in snapshot.into_iter().enumerate() {
      f(idx, Self::to_proto_pid(&pid)).await;
    }
  }

  pub async fn get(&self, index: usize) -> Option<Pid> {
    self.inner.get(index).await.map(|pid| Self::to_proto_pid(&pid))
  }
}

impl Clone for PidSet {
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
    }
  }
}

#[cfg(test)]
mod tests;
