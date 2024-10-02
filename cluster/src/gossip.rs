use crate::consensus_check::ConsensusCheck;
use crate::generated::cluster::{ClusterTopology, GossipKeyValue, Member};
use crate::gossip_state::GossipState;
use crate::member_state_delta::MemberStateDelta;
use dashmap::DashMap;
use prost_types::Any;
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Clone)]
pub struct LocalStateSender(Arc<dyn Fn(&MemberStateDelta, &Member)>);

impl Debug for LocalStateSender {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "LocalStateSender")
  }
}

pub trait GossipStateStorer {
  fn get_state(&self, key: &str) -> DashMap<String, GossipKeyValue>;
  fn set_state(&mut self, key: &str, value: &dyn prost::Message);
  fn set_map_state(&mut self, state_key: &str, map_key: &str, valeu: &dyn prost::Message);

  fn remove_map_state(&mut self, state_key: &str, map_key: &str);
  fn get_map_keys(&self, state_key: &str) -> Vec<String>;
  fn get_map_state(&self, state_key: &str, map_key: &str) -> Option<Any>;
}

pub trait GossipConsensusChecker {
  fn add_consensus_check(&mut self, id: &str, check: &ConsensusCheck);
  fn remove_consensus_check(&mut self, id: &str);
}

pub trait GossipCore {
  fn update_cluster_topology(&mut self, topology: &ClusterTopology);
  fn receive_state(&mut self, remote_state: &GossipState);

  fn send_state(&mut self, send_state_to_member: &LocalStateSender);

  fn get_member_state_delta(&self, target_member_id: &str) -> MemberStateDelta;
}

pub trait Gossip: GossipStateStorer + GossipConsensusChecker + GossipCore {}
