use crate::actor::pid::Pid;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq)]
pub enum MemberStatus {
  Alive,
  Leaving,
  Left,
  Exited,
}

#[derive(Debug, Clone)]
pub struct Member {
  pub id: String,
  pub address: String,
  pub status: MemberStatus,
}

#[derive(Debug)]
pub struct MemberList {
  members: Arc<DashMap<String, Member>>,
  status: Arc<RwLock<MemberStatus>>,
}

impl MemberList {
  pub fn new() -> Self {
    Self {
      members: Arc::new(DashMap::new()),
      status: Arc::new(RwLock::new(MemberStatus::Alive)),
    }
  }

  pub fn add_member(&self, id: String, address: String) {
    let member = Member {
      id: id.clone(),
      address,
      status: MemberStatus::Alive,
    };
    self.members.insert(id, member);
  }

  pub fn remove_member(&self, id: &str) {
    self.members.remove(id);
  }

  pub fn get_member(&self, id: &str) -> Option<Member> {
    self.members.get(id).map(|m| m.clone())
  }

  pub fn update_member_status(&self, id: &str, status: MemberStatus) {
    if let Some(mut member) = self.members.get_mut(id) {
      member.status = status;
    }
  }

  pub async fn get_status(&self) -> MemberStatus {
    self.status.read().await.clone()
  }

  pub async fn set_status(&self, status: MemberStatus) {
    *self.status.write().await = status;
  }

  pub fn get_alive_members(&self) -> Vec<Member> {
    self
      .members
      .iter()
      .filter(|m| m.status == MemberStatus::Alive)
      .map(|m| m.clone())
      .collect()
  }
}
