use crate::gossip_state::GossipState;
use std::sync::Arc;

pub struct MemberStateDelta {
  target_member_id: String,
  has_state: bool,
  state: Arc<GossipState>,
  commit_offset: Arc<dyn Fn()>,
}
