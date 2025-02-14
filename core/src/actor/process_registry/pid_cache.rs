use crate::actor::pid::Pid;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct PidCacheEntry {
  pid: Pid,
  address: String,
  last_used: Instant,
}

impl PidCacheEntry {
  pub fn new(pid: Pid, address: String) -> Self {
    Self {
      pid,
      address,
      last_used: Instant::now(),
    }
  }

  pub fn update_last_used(&mut self) {
    self.last_used = Instant::now();
  }

  pub fn is_expired(&self, ttl: Duration) -> bool {
    self.last_used.elapsed() > ttl
  }
}

#[derive(Debug)]
pub struct PidCache {
  entries: Arc<DashMap<String, PidCacheEntry>>,
  ttl: Duration,
}

impl PidCache {
  pub fn new(ttl: Duration) -> Self {
    Self {
      entries: Arc::new(DashMap::new()),
      ttl,
    }
  }

  pub fn add(&self, key: String, pid: Pid, address: String) {
    let entry = PidCacheEntry::new(pid, address);
    self.entries.insert(key, entry);
  }

  pub fn get(&self, key: &str) -> Option<Pid> {
    if let Some(mut entry) = self.entries.get_mut(key) {
      if !entry.is_expired(self.ttl) {
        entry.update_last_used();
        return Some(entry.pid.clone());
      }
      self.entries.remove(key);
    }
    None
  }

  pub fn remove_by_address(&self, address: &str) {
    self.entries.retain(|_, entry| entry.address != address);
  }

  pub fn clear(&self) {
    self.entries.clear();
  }
}
