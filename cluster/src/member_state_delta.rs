use crate::gossip_state::GossipState;
use std::sync::Arc;

#[derive(Clone)]
pub struct MemberStateDelta {
  target_member_id: String,
  has_state: bool,
  state: Arc<GossipState>,
  commit_offset: Arc<dyn Fn() + Send + Sync>,
}

impl MemberStateDelta {
  pub fn new(
    target_member_id: impl Into<String>,
    has_state: bool,
    state: Arc<GossipState>,
    commit_offset: Arc<dyn Fn() + Send + Sync>,
  ) -> Self {
    Self {
      target_member_id: target_member_id.into(),
      has_state,
      state,
      commit_offset,
    }
  }

  pub fn target_member_id(&self) -> &str {
    &self.target_member_id
  }

  pub fn has_state(&self) -> bool {
    self.has_state
  }

  pub fn state(&self) -> &Arc<GossipState> {
    &self.state
  }

  pub fn commit(&self) {
    (self.commit_offset)();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::atomic::{AtomicUsize, Ordering};

  #[test]
  fn member_state_delta_invokes_commit() {
    let state = Arc::new(GossipState {});
    let counter = Arc::new(AtomicUsize::new(0));
    let committed = counter.clone();
    let delta = MemberStateDelta::new(
      "node-1",
      true,
      state.clone(),
      Arc::new(move || {
        committed.fetch_add(1, Ordering::SeqCst);
      }),
    );

    assert_eq!(delta.target_member_id(), "node-1");
    assert!(delta.has_state());
    assert!(Arc::ptr_eq(delta.state(), &state));

    delta.commit();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
  }
}
