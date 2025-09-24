use crate::consensus_check::ConsensusCheck;
use crate::generated::cluster::{ClusterTopology, GossipKeyValue, Member};
use crate::gossip_state::GossipState;
use crate::member_state_delta::MemberStateDelta;
use dashmap::DashMap;
use prost_types::Any;
use std::fmt::Debug;
use std::sync::Arc;

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct LocalStateSender(Arc<dyn Fn(&MemberStateDelta, &Member) + Send + Sync + 'static>);

#[cfg(test)]
impl LocalStateSender {
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(&MemberStateDelta, &Member) + Send + Sync + 'static, {
    Self(Arc::new(f))
  }

  pub fn send(&self, delta: &MemberStateDelta, member: &Member) {
    (self.0)(delta, member)
  }
}

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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::consensus_check::GossipUpdater;
  use crate::gossip_state::GossipState;
  use crate::member_state_delta::MemberStateDelta;
  use std::sync::atomic::{AtomicUsize, Ordering};

  #[derive(Default)]
  struct InMemoryGossip {
    map_state: DashMap<String, DashMap<String, GossipKeyValue>>,
    checks: DashMap<String, usize>,
    topology_updates: Arc<AtomicUsize>,
    received_states: Arc<AtomicUsize>,
    deltas: DashMap<String, MemberStateDelta>,
  }

  impl GossipStateStorer for InMemoryGossip {
    fn get_state(&self, key: &str) -> DashMap<String, GossipKeyValue> {
      self
        .map_state
        .get(key)
        .map(|entry| entry.clone())
        .unwrap_or_else(DashMap::new)
    }

    fn set_state(&mut self, key: &str, _value: &dyn prost::Message) {
      self.map_state.insert(key.to_string(), DashMap::new());
    }

    fn set_map_state(&mut self, state_key: &str, map_key: &str, _value: &dyn prost::Message) {
      let entry = self.map_state.entry(state_key.to_string()).or_insert_with(DashMap::new);
      entry.insert(
        map_key.to_string(),
        GossipKeyValue {
          sequence_number: 0,
          value: None,
          local_timestamp_unix_milliseconds: 0,
        },
      );
    }

    fn remove_map_state(&mut self, state_key: &str, map_key: &str) {
      if let Some(entry) = self.map_state.get(state_key) {
        entry.remove(map_key);
      }
    }

    fn get_map_keys(&self, state_key: &str) -> Vec<String> {
      self
        .map_state
        .get(state_key)
        .map(|entry| entry.iter().map(|kv| kv.key().clone()).collect())
        .unwrap_or_default()
    }

    fn get_map_state(&self, state_key: &str, map_key: &str) -> Option<Any> {
      self
        .map_state
        .get(state_key)
        .map(|entry| entry.contains_key(map_key))
        .unwrap_or(false)
        .then(Any::default)
    }
  }

  impl GossipConsensusChecker for InMemoryGossip {
    fn add_consensus_check(&mut self, id: &str, _check: &ConsensusCheck) {
      let mut counter = self.checks.entry(id.to_string()).or_insert(0);
      *counter += 1;
    }

    fn remove_consensus_check(&mut self, id: &str) {
      self.checks.remove(id);
    }
  }

  impl GossipCore for InMemoryGossip {
    fn update_cluster_topology(&mut self, _topology: &ClusterTopology) {
      self.topology_updates.fetch_add(1, Ordering::SeqCst);
    }

    fn receive_state(&mut self, _remote_state: &GossipState) {
      self.received_states.fetch_add(1, Ordering::SeqCst);
    }

    fn send_state(&mut self, send_state_to_member: &LocalStateSender) {
      let delta = self
        .deltas
        .get("member-1")
        .map(|entry| entry.clone())
        .unwrap_or_else(|| MemberStateDelta::new("member-1", false, Arc::new(GossipState {}), Arc::new(|| {})));
      let member = Member {
        host: "127.0.0.1".into(),
        port: 0,
        id: "member-1".into(),
        kinds: vec![],
      };
      send_state_to_member.send(&delta, &member);
    }

    fn get_member_state_delta(&self, target_member_id: &str) -> MemberStateDelta {
      self
        .deltas
        .get(target_member_id)
        .map(|entry| entry.clone())
        .unwrap_or_else(|| MemberStateDelta::new(target_member_id, false, Arc::new(GossipState {}), Arc::new(|| {})))
    }
  }

  impl Gossip for InMemoryGossip {}

  #[test]
  fn local_state_sender_executes_callback() {
    let counter = Arc::new(AtomicUsize::new(0));
    let observer = counter.clone();
    let sender = LocalStateSender::new(move |_delta, _member| {
      observer.fetch_add(1, Ordering::SeqCst);
    });

    let state = Arc::new(GossipState {});
    let delta = MemberStateDelta::new("member-1", true, state, Arc::new(|| {}));
    let member = Member {
      host: "127.0.0.1".into(),
      port: 0,
      id: "member-1".into(),
      kinds: vec![],
    };
    sender.send(&delta, &member);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
  }

  #[test]
  fn gossip_trait_can_be_implemented() {
    let mut gossip = InMemoryGossip::default();
    let topology = ClusterTopology {
      topology_hash: 0,
      members: vec![],
      joined: vec![],
      left: vec![],
      blocked: vec![],
    };
    gossip.set_state("cluster", &topology);
    gossip.set_map_state("data", "node", &topology);
    assert!(gossip.get_map_keys("data").contains(&"node".to_string()));

    let updater = GossipUpdater::new(|_, _| {});
    let check = ConsensusCheck::new(vec!["cluster".into()], updater);
    gossip.add_consensus_check("check-1", &check);
    gossip.remove_consensus_check("check-1");

    gossip.update_cluster_topology(&topology);
    gossip.receive_state(&GossipState {});
    gossip.deltas.insert(
      "member-1".into(),
      MemberStateDelta::new("member-1", true, Arc::new(GossipState {}), Arc::new(|| {})),
    );
    let sender = LocalStateSender::new(|_, _| {});
    gossip.send_state(&sender);
    let delta = gossip.get_member_state_delta("member-1");
    assert!(delta.has_state());
  }
}
