use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::actor::actor::sender_middleware_chain::SenderMiddlewareChain;

#[derive(Clone)]
pub struct SenderMiddleware(Arc<dyn Fn(SenderMiddlewareChain) -> SenderMiddlewareChain + Send + Sync + 'static>);

unsafe impl Send for SenderMiddleware {}
unsafe impl Sync for SenderMiddleware {}

impl Debug for SenderMiddleware {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "SenderMiddleware")
  }
}

impl PartialEq for SenderMiddleware {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for SenderMiddleware {}

impl std::hash::Hash for SenderMiddleware {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(SenderMiddlewareChain) -> SenderMiddlewareChain).hash(state);
  }
}

impl SenderMiddleware {
  pub fn new(f: impl Fn(SenderMiddlewareChain) -> SenderMiddlewareChain + Send + Sync + 'static) -> Self {
    SenderMiddleware(Arc::new(f))
  }

  pub fn run(&self, next: SenderMiddlewareChain) -> SenderMiddlewareChain {
    (self.0)(next)
  }
}
