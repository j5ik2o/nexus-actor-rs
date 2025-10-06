use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use nexus_actor_core_rs::context::CoreSupervisorStrategyHandle;
use nexus_actor_core_rs::supervisor::CoreSupervisorStrategy;

#[derive(Clone)]
pub struct SupervisorStrategyHandle(CoreSupervisorStrategyHandle);

impl SupervisorStrategyHandle {
  pub fn new_arc(strategy: CoreSupervisorStrategyHandle) -> Self {
    Self(strategy)
  }

  pub fn new<S>(strategy: S) -> Self
  where
    S: CoreSupervisorStrategy + Send + Sync + 'static, {
    Self(Arc::new(strategy))
  }

  pub fn core_strategy(&self) -> CoreSupervisorStrategyHandle {
    self.0.clone()
  }
}

impl PartialEq for SupervisorStrategyHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for SupervisorStrategyHandle {}

impl Hash for SupervisorStrategyHandle {
  fn hash<H: Hasher>(&self, state: &mut H) {
    (Arc::as_ptr(&self.0) as *const ()).hash(state);
  }
}

impl fmt::Debug for SupervisorStrategyHandle {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_tuple("SupervisorStrategyHandle")
      .field(&(Arc::as_ptr(&self.0) as *const ()))
      .finish()
  }
}

#[cfg(test)]
mod tests;
