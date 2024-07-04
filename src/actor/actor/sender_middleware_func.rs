use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::actor::actor::sender_middleware_chain_func::SenderMiddlewareChainFunc;

#[derive(Clone)]
pub struct SenderMiddlewareFunc(Arc<dyn Fn(SenderMiddlewareChainFunc) -> SenderMiddlewareChainFunc + Send + Sync>);

impl Debug for SenderMiddlewareFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SenderMiddleware")
  }
}

impl PartialEq for SenderMiddlewareFunc {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for SenderMiddlewareFunc {}

impl std::hash::Hash for SenderMiddlewareFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(SenderMiddlewareChainFunc) -> SenderMiddlewareChainFunc).hash(state);
  }
}

impl SenderMiddlewareFunc {
  pub fn new(f: impl Fn(SenderMiddlewareChainFunc) -> SenderMiddlewareChainFunc + Send + Sync + 'static) -> Self {
    SenderMiddlewareFunc(Arc::new(f))
  }

  pub fn run(&self, next: SenderMiddlewareChainFunc) -> SenderMiddlewareChainFunc {
    (self.0)(next)
  }
}
