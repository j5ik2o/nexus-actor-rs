use std::collections::HashMap;
use std::hash::Hasher;
use std::sync::RwLock;

use fnv::FnvHasher;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClusterMember {
  pub address: String,
  pub kinds: Vec<String>,
}

impl ClusterMember {
  pub fn new(address: impl Into<String>, kinds: impl IntoIterator<Item = String>) -> Self {
    Self {
      address: address.into(),
      kinds: kinds.into_iter().collect(),
    }
  }

  pub fn has_kind(&self, kind: &str) -> bool {
    self.kinds.iter().any(|k| k == kind)
  }
}

#[derive(Debug, Default)]
pub struct Rendezvous {
  members: RwLock<Vec<ClusterMember>>,             // all members
  kind_index: RwLock<HashMap<String, Vec<usize>>>, // kind -> member indices
}

impl Rendezvous {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn update_members(&self, members: Vec<ClusterMember>) {
    let mut kind_index = HashMap::new();
    for (idx, member) in members.iter().enumerate() {
      for kind in &member.kinds {
        kind_index.entry(kind.clone()).or_insert_with(Vec::new).push(idx);
      }
    }

    {
      let mut guard = self.members.write().unwrap();
      *guard = members;
    }
    {
      let mut guard = self.kind_index.write().unwrap();
      *guard = kind_index;
    }
  }

  pub fn owner_for_kind_identity(&self, kind: &str, identity: &str) -> Option<String> {
    let members_guard = self.members.read().unwrap();
    let kind_index_guard = self.kind_index.read().unwrap();
    let indices = kind_index_guard.get(kind)?;
    let seed = identity.as_bytes();

    let mut max_score = 0u32;
    let mut owner: Option<&ClusterMember> = None;

    for &idx in indices {
      if let Some(member) = members_guard.get(idx) {
        let mut hasher = FnvHasher::default();
        hasher.write(seed);
        hasher.write(member.address.as_bytes());
        let score = hasher.finish() as u32;
        if score > max_score || owner.is_none() {
          max_score = score;
          owner = Some(member);
        }
      }
    }

    owner.map(|member| member.address.clone())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rendezvous_selects_consistent_owner() {
    let rendezvous = Rendezvous::new();
    rendezvous.update_members(vec![
      ClusterMember::new("node-a", vec!["greeter".to_string()]),
      ClusterMember::new("node-b", vec!["greeter".to_string()]),
      ClusterMember::new("node-c", vec!["greeter".to_string()]),
    ]);

    let owner1 = rendezvous.owner_for_kind_identity("greeter", "alpha").unwrap();
    let owner2 = rendezvous.owner_for_kind_identity("greeter", "alpha").unwrap();
    assert_eq!(owner1, owner2);
  }

  #[test]
  fn rendezvous_filters_by_kind() {
    let rendezvous = Rendezvous::new();
    rendezvous.update_members(vec![
      ClusterMember::new("node-a", vec!["greeter".to_string()]),
      ClusterMember::new("node-b", vec!["worker".to_string()]),
    ]);

    let owner = rendezvous.owner_for_kind_identity("worker", "alpha").unwrap();
    assert_eq!(owner, "node-b");
  }

  #[test]
  fn rendezvous_returns_none_when_no_members() {
    let rendezvous = Rendezvous::new();
    assert!(rendezvous.owner_for_kind_identity("greeter", "id").is_none());
  }
}
