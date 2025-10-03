use dashmap::DashMap;
use once_cell::sync::Lazy;

use crate::actor::actor_system::ActorSystemId;
use crate::actor::context::context_handle::{ContextHandle, WeakContextHandle};

#[derive(Clone, Default)]
pub struct ContextRegistry;

static REGISTRY: Lazy<DashMap<(ActorSystemId, String), WeakContextHandle>> = Lazy::new(DashMap::new);

impl ContextRegistry {
  pub fn register(system_id: ActorSystemId, pid: &str, handle: &ContextHandle) {
    REGISTRY.insert((system_id, pid.to_string()), handle.downgrade());
  }

  pub fn unregister(system_id: ActorSystemId, pid: &str) {
    REGISTRY.remove(&(system_id, pid.to_string()));
  }

  pub fn get(system_id: ActorSystemId, pid: &str) -> Option<ContextHandle> {
    let key = (system_id, pid.to_string());
    if let Some(ref_map) = REGISTRY.get(&key) {
      if let Some(handle) = ref_map.value().upgrade() {
        return Some(handle);
      }
    }
    REGISTRY.remove(&key);
    None
  }
}
