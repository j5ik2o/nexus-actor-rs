use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{ActorSystem, ActorSystemId, WeakActorSystem};

static ACTOR_SYSTEM_REGISTRY: Lazy<DashMap<ActorSystemId, WeakActorSystem>> = Lazy::new(DashMap::new);
static ACTOR_SYSTEM_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn next_actor_system_id() -> ActorSystemId {
  ACTOR_SYSTEM_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

pub fn register_actor_system(id: ActorSystemId, system: &ActorSystem) {
  ACTOR_SYSTEM_REGISTRY.insert(id, system.downgrade());
}

pub fn unregister_actor_system(id: ActorSystemId) {
  ACTOR_SYSTEM_REGISTRY.remove(&id);
}

pub fn with_actor_system<F, R>(id: ActorSystemId, f: F) -> Option<R>
where
  F: FnOnce(ActorSystem) -> R, {
  let entry = ACTOR_SYSTEM_REGISTRY.get(&id)?;
  let weak = entry.value().clone();
  drop(entry);
  weak.upgrade().map(f)
}
