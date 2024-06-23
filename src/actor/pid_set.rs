use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::actor::actor::Pid;
use crate::actor::pid::ExtendedPid;

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

#[cfg(test)]
mod tests {
  use crate::actor::actor_system::ActorSystem;

  use super::*;

  async fn new_pid(system: ActorSystem, address: &str, id: &str, request_id: u32) -> ExtendedPid {
    let pid = Pid {
      address: address.to_string(),
      id: id.to_string(),
      request_id,
    };
    ExtendedPid::new(pid, system)
  }

  #[tokio::test]
  async fn test_pid_set_empty() {
    let s = PidSet::new(&[]).await;
    assert!(s.is_empty().await);
  }

  #[tokio::test]
  async fn test_pid_set_clear() {
    let system = ActorSystem::new(&[]).await;
    let mut s = PidSet::new(&[]).await;
    s.add(new_pid(system.clone(), "local", "p1", 0).await).await;
    s.add(new_pid(system.clone(), "local", "p2", 0).await).await;
    s.add(new_pid(system.clone(), "local", "p3", 0).await).await;
    assert_eq!(3, s.len().await);
    s.clear().await;
    assert!(s.is_empty().await);
    // assert_eq!(0, s.pids.len());
  }

  #[tokio::test]
  async fn test_pid_set_add_small() {
    let system = ActorSystem::new(&[]).await;
    let mut s = PidSet::new(&[]).await;
    let p1 = new_pid(system.clone(), "local", "p1", 0).await;
    s.add(p1).await;
    assert!(!s.is_empty().await);
    let p1 = new_pid(system, "local", "p1", 0).await;
    s.add(p1).await;
    assert_eq!(1, s.len().await);
  }

  #[tokio::test]
  async fn test_pid_set_values() {
    let system = ActorSystem::new(&[]).await;
    let mut s = PidSet::new(&[]).await;
    s.add(new_pid(system.clone(), "local", "p1", 0).await).await;
    s.add(new_pid(system.clone(), "local", "p2", 0).await).await;
    s.add(new_pid(system.clone(), "local", "p3", 0).await).await;
    assert!(!s.is_empty().await);

    let r = s.to_vec().await;
    assert_eq!(3, r.len());
  }

  #[tokio::test]
  async fn test_pid_set_add_map() {
    let system = ActorSystem::new(&[]).await;
    let mut s = PidSet::new(&[]).await;
    let p1 = new_pid(system.clone(), "local", "p1", 0).await;
    s.add(p1).await;
    assert!(!s.is_empty().await);
    let p1 = new_pid(system, "local", "p1", 0).await;
    s.add(p1).await;
    assert_eq!(1, s.len().await);
  }

  #[tokio::test]
  async fn test_pid_set_add_remove() {
    let system = ActorSystem::new(&[]).await;
    let mut pids = vec![];
    for i in 0..1000 {
      let pid = new_pid(system.clone(), "local", &format!("p{}", i), 0).await;
      pids.push(pid);
    }

    let mut s = PidSet::new(&[]).await;
    for pid in &pids {
      s.add(pid.clone()).await;
    }
    assert_eq!(1000, s.len().await);

    for pid in &pids {
      assert!(s.remove(pid).await);
    }
    assert!(s.is_empty().await);
  }
}
