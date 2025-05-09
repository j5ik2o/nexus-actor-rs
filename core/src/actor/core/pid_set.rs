use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::generated::actor::Pid;

#[derive(Debug, Clone)]
pub struct PidSet {
  pids: Arc<RwLock<Vec<Pid>>>,
  lookup: Arc<RwLock<HashMap<String, Pid>>>,
}

impl PidSet {
  pub fn key(&self, pid: &Pid) -> String {
    pid.id.to_string()
  }

  pub async fn new() -> Self {
    Self::new_with_pids(&[]).await
  }

  pub async fn new_with_pids(pids: &[Pid]) -> Self {
    let mut set = PidSet {
      pids: Arc::new(RwLock::new(Vec::new())),
      lookup: Arc::new(RwLock::new(HashMap::new())),
    };
    for pid in pids {
      set.add(pid.clone()).await;
    }
    set
  }

  pub async fn ensure_init(&mut self) {
    let mut mg = self.lookup.write().await;
    if mg.is_empty() {
      *mg = HashMap::new();
    }
  }

  pub async fn index_of(&self, v: &Pid) -> Option<usize> {
    let pids_mg = self.pids.read().await;
    pids_mg.iter().position(|pid| *v == *pid)
  }

  pub async fn contains(&self, v: &Pid) -> bool {
    let mg = self.lookup.read().await;
    mg.contains_key(&self.key(v))
  }

  pub async fn add(&mut self, v: Pid) {
    self.ensure_init().await;
    if self.contains(&v).await {
      return;
    }
    let key = self.key(&v);
    {
      let mut lookup_mg = self.lookup.write().await;
      lookup_mg.insert(key, v.clone());
    }
    {
      let mut pids_mg = self.pids.write().await;
      pids_mg.push(v);
    }
  }

  pub async fn remove(&mut self, v: &Pid) -> bool {
    self.ensure_init().await;
    if let Some(i) = self.index_of(v).await {
      {
        let mut lookup_mg = self.lookup.write().await;
        lookup_mg.remove(&self.key(v));
      }
      {
        let mut pids_mg = self.pids.write().await;
        pids_mg.remove(i);
      }
      true
    } else {
      false
    }
  }

  pub async fn len(&self) -> usize {
    let pids_mg = self.pids.read().await;
    pids_mg.len()
  }

  pub async fn clear(&mut self) {
    {
      let mut pids_mg = self.pids.write().await;
      pids_mg.clear();
    }
    {
      let mut lookup_mg = self.lookup.write().await;
      lookup_mg.clear();
    }
  }

  pub async fn is_empty(&self) -> bool {
    self.len().await == 0
  }

  pub async fn to_vec(&self) -> Vec<Pid> {
    let pids_mg = self.pids.read().await;
    pids_mg.clone()
  }

  pub async fn for_each<F>(&self, mut f: F)
  where
    F: FnMut(usize, &Pid), {
    let pids_mg = self.pids.read().await;
    for (i, pid) in pids_mg.iter().enumerate() {
      f(i, pid);
    }
  }

  pub async fn get(&self, index: usize) -> Option<Pid> {
    let pids_mg = self.pids.read().await;
    pids_mg.get(index).cloned()
  }
}
