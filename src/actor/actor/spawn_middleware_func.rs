use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::actor::actor::spawn_func::SpawnFunc;

#[derive(Clone)]
pub struct SpawnMiddlewareFunc(Arc<dyn Fn(SpawnFunc) -> SpawnFunc + Send + Sync>);

impl Debug for SpawnMiddlewareFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SpawnMiddleware")
  }
}

impl PartialEq for SpawnMiddlewareFunc {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for SpawnMiddlewareFunc {}

impl std::hash::Hash for SpawnMiddlewareFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(SpawnFunc) -> SpawnFunc).hash(state);
  }
}

impl SpawnMiddlewareFunc {
  pub fn new(f: impl Fn(SpawnFunc) -> SpawnFunc + Send + Sync + 'static) -> Self {
    SpawnMiddlewareFunc(Arc::new(f))
  }

  pub fn run(&self, next: SpawnFunc) -> SpawnFunc {
    self.0(next)
  }
}
