use std::sync::Arc;

use dashmap::DashMap;
use nexus_actor_std_rs::actor::core::PidSet;
use nexus_actor_std_rs::generated::actor::Pid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchStat {
  pub changed: bool,
  pub watchers: usize,
}

impl WatchStat {
  pub const fn unchanged(watchers: usize) -> Self {
    Self {
      changed: false,
      watchers,
    }
  }

  pub const fn changed(watchers: usize) -> Self {
    Self {
      changed: true,
      watchers,
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct WatchRegistry {
  inner: Arc<DashMap<String, PidSet>>,
}

impl WatchRegistry {
  pub fn new() -> Self {
    Self {
      inner: Arc::new(DashMap::new()),
    }
  }

  pub fn inner(&self) -> Arc<DashMap<String, PidSet>> {
    self.inner.clone()
  }

  pub fn len(&self) -> usize {
    self.inner.len()
  }

  pub fn is_empty(&self) -> bool {
    self.inner.is_empty()
  }

  pub fn get_pid_set(&self, watcher_id: &str) -> Option<PidSet> {
    self.inner.get(watcher_id).map(|entry| entry.value().clone())
  }

  pub fn snapshot(&self) -> Vec<(String, PidSet)> {
    self
      .inner
      .iter()
      .map(|entry| (entry.key().clone(), entry.value().clone()))
      .collect()
  }

  fn ensure_pid_set(&self, watcher_id: &str) -> PidSet {
    if let Some(existing) = self.get_pid_set(watcher_id) {
      existing
    } else {
      let pid_set = PidSet::new();
      let entry = self.inner.entry(watcher_id.to_string()).or_insert(pid_set);
      entry.clone()
    }
  }

  pub async fn watch(&self, watcher_id: &str, watchee: Pid) -> WatchStat {
    let pid_set = self.ensure_pid_set(watcher_id);
    let existed = pid_set.contains(&watchee).await;
    if !existed {
      pid_set.add(watchee).await;
    }
    let watchers = pid_set.len().await;
    if existed {
      WatchStat::unchanged(watchers)
    } else {
      WatchStat::changed(watchers)
    }
  }

  pub async fn unwatch(&self, watcher_id: &str, watchee: &Pid) -> Option<WatchStat> {
    if let Some(pid_set) = self.get_pid_set(watcher_id) {
      let removed = pid_set.remove(watchee).await;
      let watchers = pid_set.len().await;
      if removed && watchers == 0 {
        self.inner.remove(watcher_id);
      }
      Some(WatchStat {
        changed: removed,
        watchers,
      })
    } else {
      None
    }
  }

  pub async fn remove_watchee(&self, watcher_id: &str, watchee: Option<&Pid>) -> Option<WatchStat> {
    match watchee {
      Some(pid) => self.unwatch(watcher_id, pid).await,
      None => {
        let stat = if let Some(pid_set) = self.get_pid_set(watcher_id) {
          let watchers = pid_set.len().await;
          if watchers == 0 {
            self.inner.remove(watcher_id);
            Some(WatchStat::changed(0))
          } else {
            Some(WatchStat::unchanged(watchers))
          }
        } else {
          None
        };
        stat
      }
    }
  }

  pub async fn prune_if_empty(&self, watcher_id: &str) -> bool {
    if let Some(pid_set) = self.get_pid_set(watcher_id) {
      if pid_set.is_empty().await {
        self.inner.remove(watcher_id);
        return true;
      }
    }
    false
  }

  pub async fn clear(&self) {
    self.inner.clear();
  }
}
