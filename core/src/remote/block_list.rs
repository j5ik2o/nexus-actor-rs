use dashmap::DashSet;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct BlockList {
  blocked_members: Arc<DashSet<String>>,
}

impl BlockList {
  pub fn new() -> Self {
    BlockList {
      blocked_members: Arc::new(DashSet::new()),
    }
  }

  pub async fn block(&self, member: String) {
    self.blocked_members.insert(member);
  }

  pub async fn block_multi(&self, members: impl IntoIterator<Item = String>) {
    for member in members {
      self.blocked_members.insert(member);
    }
  }

  pub async fn unblock(&self, member: String) {
    self.blocked_members.remove(&member);
  }

  pub async fn unblock_multi(&self, members: impl IntoIterator<Item = String>) {
    for member in members {
      self.blocked_members.remove(&member);
    }
  }

  pub async fn is_blocked(&self, member: &str) -> bool {
    self.blocked_members.contains(member)
  }

  pub async fn clear(&self) {
    self.blocked_members.clear();
  }

  pub async fn get_blocked_members(&self) -> Arc<DashSet<String>> {
    self.blocked_members.clone()
  }
}
