use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::actor::core::spawner::Spawner;

#[derive(Clone)]
pub struct SpawnMiddleware(Arc<dyn Fn(Spawner) -> Spawner + Send + Sync + 'static>);

unsafe impl Send for SpawnMiddleware {}
unsafe impl Sync for SpawnMiddleware {}

impl SpawnMiddleware {
  pub fn new(f: impl Fn(Spawner) -> Spawner + Send + Sync + 'static) -> Self {
    SpawnMiddleware(Arc::new(f))
  }

  pub fn run(&self, next: Spawner) -> Spawner {
    (self.0)(next)
  }
}

impl Debug for SpawnMiddleware {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SpawnMiddleware")
  }
}

impl PartialEq for SpawnMiddleware {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for SpawnMiddleware {}

impl std::hash::Hash for SpawnMiddleware {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (Arc::as_ptr(&self.0) as *const ()).hash(state);
  }
}

static_assertions::assert_impl_all!(SpawnMiddleware: Send, Sync);
