use std::fmt::{Debug, Formatter};

use nexus_actor_core_rs::context::CoreSpawnMiddleware;

use crate::actor::core::spawner::Spawner;

#[derive(Clone)]
pub struct SpawnMiddleware(CoreSpawnMiddleware<Spawner>);

unsafe impl Send for SpawnMiddleware {}
unsafe impl Sync for SpawnMiddleware {}

impl SpawnMiddleware {
  pub fn new(f: impl Fn(Spawner) -> Spawner + Send + Sync + 'static) -> Self {
    SpawnMiddleware(CoreSpawnMiddleware::new(f))
  }

  pub fn run(&self, next: Spawner) -> Spawner {
    self.0.run(next)
  }

  pub fn as_core(&self) -> &CoreSpawnMiddleware<Spawner> {
    &self.0
  }

  pub fn into_core(self) -> CoreSpawnMiddleware<Spawner> {
    self.0
  }
}

impl Debug for SpawnMiddleware {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SpawnMiddleware")
  }
}

impl PartialEq for SpawnMiddleware {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl Eq for SpawnMiddleware {}

impl std::hash::Hash for SpawnMiddleware {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.0.hash(state);
  }
}

static_assertions::assert_impl_all!(SpawnMiddleware: Send, Sync);
