use dashmap::DashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct BlockList {
  blocked_members: Arc<RwLock<DashSet<String>>>,
}

impl BlockList {
  pub fn new() -> Self {
    BlockList {
      blocked_members: Arc::new(RwLock::new(DashSet::new())),
    }
  }

  pub async fn block(&self, member: String) {
    self.blocked_members.write().await.insert(member);
  }

  pub async fn block_multi(&self, members: impl IntoIterator<Item = String>) {
    let mg = self.blocked_members.write().await;
    for member in members {
      mg.insert(member);
    }
  }

  pub async fn unblock(&self, member: String) {
    self.blocked_members.write().await.remove(&member);
  }

  pub async fn unblock_multi(&self, members: impl IntoIterator<Item = String>) {
    let mg = self.blocked_members.write().await;
    for member in members {
      mg.remove(&member);
    }
  }

  pub async fn is_blocked(&self, member: &str) -> bool {
    self.blocked_members.read().await.contains(member)
  }

  pub async fn clear(&self) {
    self.blocked_members.write().await.clear();
  }

  pub async fn get_blocked_members(&self) -> DashSet<String> {
    self.blocked_members.read().await.clone()
  }
}
