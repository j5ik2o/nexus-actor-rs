use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::actor::actor::pid::ExtendedPid;

#[derive(Debug, Clone)]
pub(crate) struct PidSet {
  pids: Arc<Mutex<Vec<ExtendedPid>>>,
  lookup: Arc<Mutex<HashMap<String, ExtendedPid>>>,
}

impl PidSet {
  pub(crate) fn key(&self, pid: &ExtendedPid) -> String {
    format!("{}:{}", pid.address(), pid.id())
  }

  pub(crate) async fn new(pids: &[ExtendedPid]) -> Self {
    let mut set = PidSet {
      pids: Arc::new(Mutex::new(Vec::new())),
      lookup: Arc::new(Mutex::new(HashMap::new())),
    };
    for pid in pids {
      set.add(pid.clone()).await;
    }
    set
  }

  pub(crate) async fn ensure_init(&mut self) {
    let mut mg = self.lookup.lock().await;
    if mg.is_empty() {
      *mg = HashMap::new();
    }
  }

  pub(crate) async fn index_of(&self, v: &ExtendedPid) -> Option<usize> {
    let pids_mg = self.pids.lock().await;
    pids_mg.iter().position(|pid| *v == *pid)
  }

  pub(crate) async fn contains(&self, v: &ExtendedPid) -> bool {
    let mg = self.lookup.lock().await;
    mg.contains_key(&self.key(v))
  }

  pub(crate) async fn add(&mut self, v: ExtendedPid) {
    self.ensure_init().await;
    if self.contains(&v).await {
      return;
    }
    let key = self.key(&v);
    {
      let mut lookup_mg = self.lookup.lock().await;
      lookup_mg.insert(key, v.clone());
    }
    {
      let mut pids_mg = self.pids.lock().await;
      pids_mg.push(v);
    }
  }

  pub(crate) async fn remove(&mut self, v: &ExtendedPid) -> bool {
    self.ensure_init().await;
    if let Some(i) = self.index_of(v).await {
      {
        let mut lookup_mg = self.lookup.lock().await;
        lookup_mg.remove(&self.key(v));
      }
      {
        let mut pids_mg = self.pids.lock().await;
        pids_mg.remove(i);
      }
      true
    } else {
      false
    }
  }

  pub(crate) async fn len(&self) -> usize {
    let pids_mg = self.pids.lock().await;
    pids_mg.len()
  }

  pub(crate) async fn clear(&mut self) {
    {
      let mut pids_mg = self.pids.lock().await;
      pids_mg.clear();
    }
    {
      let mut lookup_mg = self.lookup.lock().await;
      lookup_mg.clear();
    }
  }

  pub(crate) async fn is_empty(&self) -> bool {
    self.len().await == 0
  }

  pub(crate) async fn to_vec(&self) -> Vec<ExtendedPid> {
    let pids_mg = self.pids.lock().await;
    pids_mg.clone()
  }

  pub(crate) async fn for_each<F>(&self, mut f: F)
  where
    F: FnMut(usize, &ExtendedPid), {
    let pids_mg = self.pids.lock().await;
    for (i, pid) in pids_mg.iter().enumerate() {
      f(i, pid);
    }
  }

  pub(crate) async fn get(&self, index: usize) -> Option<ExtendedPid> {
    let pids_mg = self.pids.lock().await;
    pids_mg.get(index).cloned()
  }
}
