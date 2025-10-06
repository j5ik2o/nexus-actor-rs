use dashmap::DashSet;
use std::sync::Arc;

use crate::BlockListStore;

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

impl BlockListStore for BlockList {
  fn is_blocked(&self, system: &str) -> bool {
    self.blocked_members.contains(system)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn block_list_operations_cover_all_paths() {
    let block_list = BlockList::new();

    block_list.block("node-a".into()).await;
    block_list
      .block_multi(vec!["node-b".to_string(), "node-c".to_string()])
      .await;
    assert!(block_list.is_blocked("node-a").await);
    assert!(block_list.is_blocked("node-b").await);

    block_list.unblock("node-a".into()).await;
    block_list
      .unblock_multi(vec!["node-b".to_string(), "node-c".to_string()])
      .await;

    assert!(!block_list.is_blocked("node-a").await);

    block_list.block("node-d".into()).await;
    block_list.clear().await;

    let members = block_list.get_blocked_members().await;
    assert!(members.is_empty());
  }
}
