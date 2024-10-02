use crate::gossip_state::GossipState;
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct GossipUpdater(Arc<dyn Fn(&GossipState, DashMap<String, ()>)>);

pub struct ConsensusCheck {
  affected_keys: Vec<String>,
  check: GossipUpdater,
}
