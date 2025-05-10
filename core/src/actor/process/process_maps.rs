use crate::actor::process::ProcessHandle;
use dashmap::DashMap;
use siphasher::sip::SipHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub(crate) struct ProcessMaps {
  local_pids: Vec<DashMap<String, ProcessHandle>>,
}

const CAPACITY: usize = 1024;

impl ProcessMaps {
  pub fn new() -> Self {
    let mut local_pids = Vec::with_capacity(CAPACITY);
    for _ in 0..CAPACITY {
      local_pids.push(DashMap::new());
    }
    Self {
      local_pids,
    }
  }

  pub(crate) fn get_map(&self, key: &str) -> &DashMap<String, ProcessHandle> {
    let mut hasher = SipHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();
    let index = (hash % 1024) as usize;
    &self.local_pids[index]
  }
}
