use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::actor::actor_system::ActorSystemId;
use crate::actor::context::context_handle::{ContextHandle, WeakContextHandle};

#[derive(Clone, Default)]
pub struct ContextRegistry;

struct ContextRegistrationGuard {
  system_id: ActorSystemId,
  pid: String,
  active: AtomicBool,
}

impl ContextRegistrationGuard {
  fn new(system_id: ActorSystemId, pid: String) -> Self {
    Self {
      system_id,
      pid,
      active: AtomicBool::new(true),
    }
  }

  fn release(&self) {
    if self.active.swap(false, Ordering::Relaxed) {
      REGISTRY.remove(&(self.system_id, self.pid.clone()));
    }
  }
}

impl Drop for ContextRegistrationGuard {
  fn drop(&mut self) {
    self.release();
  }
}

static REGISTRY: Lazy<DashMap<(ActorSystemId, String), WeakContextHandle>> = Lazy::new(DashMap::new);
static REGISTRATION_GUARDS: Lazy<DashMap<(ActorSystemId, String), ContextRegistrationGuard>> = Lazy::new(DashMap::new);

impl ContextRegistry {
  pub fn register(system_id: ActorSystemId, pid: &str, handle: &ContextHandle) {
    let pid_owned = pid.to_string();
    let key = (system_id, pid_owned.clone());

    if let Some((_, guard)) = REGISTRATION_GUARDS.remove(&key) {
      guard.release();
    }

    REGISTRY.insert(key.clone(), handle.downgrade());
    REGISTRATION_GUARDS.insert(key, ContextRegistrationGuard::new(system_id, pid_owned));
  }

  pub fn unregister(system_id: ActorSystemId, pid: &str) {
    let key = (system_id, pid.to_string());
    if let Some((_, guard)) = REGISTRATION_GUARDS.remove(&key) {
      guard.release();
    } else {
      REGISTRY.remove(&key);
    }
  }

  pub fn get(system_id: ActorSystemId, pid: &str) -> Option<ContextHandle> {
    let key = (system_id, pid.to_string());
    let handle = REGISTRY.get(&key).and_then(|entry| entry.value().upgrade());

    if let Some((_, guard)) = REGISTRATION_GUARDS.remove(&key) {
      guard.release();
    } else {
      REGISTRY.remove(&key);
    }

    handle
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::actor::actor_system::ActorSystem;
  use crate::actor::context::mock_context::MockContext;
  use crate::actor::context::InfoPart;

  #[tokio::test]
  async fn get_consumes_registration() {
    let system = ActorSystem::new().await.unwrap();
    let context = ContextHandle::new(MockContext::new(system.clone()));
    let system_id = system.system_id();
    let pid = "guarded-context";

    ContextRegistry::register(system_id, pid, &context);

    let retrieved = ContextRegistry::get(system_id, pid).expect("context should be retrievable once");
    // Ensure the context handle is still functional by accessing its actor system.
    let actor_system = retrieved.get_actor_system().await;
    assert_eq!(actor_system.system_id(), system_id);

    assert!(ContextRegistry::get(system_id, pid).is_none());
  }

  #[tokio::test]
  async fn unregister_cleans_registry() {
    let system = ActorSystem::new().await.unwrap();
    let context = ContextHandle::new(MockContext::new(system.clone()));
    let system_id = system.system_id();
    let pid = "guarded-unregister";

    ContextRegistry::register(system_id, pid, &context);
    ContextRegistry::unregister(system_id, pid);

    assert!(ContextRegistry::get(system_id, pid).is_none());
  }
}
