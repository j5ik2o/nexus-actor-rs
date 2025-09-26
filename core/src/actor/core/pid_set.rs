use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::generated::actor::Pid;

#[derive(Debug, Default)]
struct PidSetInner {
  pids: RwLock<Vec<Pid>>,
  lookup: RwLock<HashMap<String, Pid>>,
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
    let set = Self::new();
    for pid in pids {
      set.add(pid.clone());
    }
    set
  }

  fn key(pid: &Pid) -> String {
    format!("{}::{}", pid.address, pid.id)
  }

  pub fn contains(&self, pid: &Pid) -> bool {
    let lookup = self.inner.lookup.read();
    lookup.contains_key(&Self::key(pid))
  }

  pub fn add(&self, pid: Pid) {
    if self.contains(&pid) {
      return;
    }
    let key = Self::key(&pid);
    {
      let mut lookup = self.inner.lookup.write();
      lookup.insert(key, pid.clone());
    }
    {
      let mut pids = self.inner.pids.write();
      pids.push(pid);
    }
  }

  pub fn remove(&self, pid: &Pid) -> bool {
    if let Some(index) = self.index_of(pid) {
      {
        let mut lookup = self.inner.lookup.write();
        lookup.remove(&Self::key(pid));
      }
      {
        let mut pids = self.inner.pids.write();
        pids.remove(index);
      }
      true
    } else {
      false
    }
  }

  pub fn len(&self) -> usize {
    let pids = self.inner.pids.read();
    pids.len()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn clear(&self) {
    self.inner.lookup.write().clear();
    self.inner.pids.write().clear();
  }

  pub fn to_vec(&self) -> Vec<Pid> {
    self.inner.pids.read().clone()
  }

  pub fn for_each<F>(&self, mut f: F)
  where
    F: FnMut(usize, &Pid),
  {
    let pids = self.inner.pids.read();
    for (idx, pid) in pids.iter().enumerate() {
      f(idx, pid);
    }
  }

  pub fn get(&self, index: usize) -> Option<Pid> {
    let pids = self.inner.pids.read();
    pids.get(index).cloned()
  }

  fn index_of(&self, pid: &Pid) -> Option<usize> {
    let pids = self.inner.pids.read();
    pids.iter().position(|candidate| candidate == pid)
  }
}

#[cfg(test)]
mod tests;
