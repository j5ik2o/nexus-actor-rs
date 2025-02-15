use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::actor::spawner::Spawner;

#[derive(Clone)]
pub struct SpawnMiddleware(Arc<dyn Fn(Spawner) -> Spawner + Send + Sync + 'static>);

unsafe impl Send for SpawnMiddleware {}
unsafe impl Sync for SpawnMiddleware {}

impl Debug for SpawnMiddleware {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SpawnMiddleware")
  }
}

impl PartialEq for SpawnMiddleware {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for SpawnMiddleware {}

impl std::hash::Hash for SpawnMiddleware {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(Spawner) -> Spawner).hash(state);
  }
}

impl SpawnMiddleware {
  pub fn new(f: impl Fn(Spawner) -> Spawner + Send + Sync + 'static) -> Self {
    SpawnMiddleware(Arc::new(f))
  }

  pub fn run(&self, next: Spawner) -> Spawner {
    self.0(next)
  }
}

static_assertions::assert_impl_all!(SpawnMiddleware: Send, Sync);
