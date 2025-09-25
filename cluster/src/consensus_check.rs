use crate::gossip_state::GossipState;
use dashmap::DashMap;
use std::sync::Arc;

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct GossipUpdater(Arc<dyn Fn(&GossipState, DashMap<String, ()>) + Send + Sync + 'static>);

#[cfg(test)]
impl GossipUpdater {
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(&GossipState, DashMap<String, ()>) + Send + Sync + 'static,
  {
    Self(Arc::new(f))
  }

  pub fn run(&self, state: &GossipState, updates: DashMap<String, ()>) {
    (self.0)(state, updates);
  }
}

pub struct ConsensusCheck {
  affected_keys: Vec<String>,
  check: GossipUpdater,
}

#[cfg(test)]
impl ConsensusCheck {
  pub fn new(affected_keys: Vec<String>, check: GossipUpdater) -> Self {
    Self { affected_keys, check }
  }

  pub fn affected_keys(&self) -> &[String] {
    &self.affected_keys
  }

  pub fn updater(&self) -> &GossipUpdater {
    &self.check
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn consensus_check_invokes_updater() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let invoked = Arc::new(AtomicBool::new(false));
    let flag = invoked.clone();
    let updater = GossipUpdater::new(move |_state: &GossipState, _changes: DashMap<String, ()>| {
      flag.store(true, Ordering::SeqCst);
    });

    let check = ConsensusCheck::new(vec!["member/a".into()], updater.clone());
    check.updater().run(&GossipState {}, DashMap::new());

    assert!(invoked.load(Ordering::SeqCst));
    assert_eq!(check.affected_keys(), &["member/a".to_string()]);
  }
}
