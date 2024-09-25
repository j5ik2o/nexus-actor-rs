use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::actor::actor::receiver_middleware_chain::ReceiverMiddlewareChain;

#[derive(Clone)]
pub struct ReceiverMiddleware(Arc<dyn Fn(ReceiverMiddlewareChain) -> ReceiverMiddlewareChain + Send + Sync + 'static>);

unsafe impl Send for ReceiverMiddleware {}
unsafe impl Sync for ReceiverMiddleware {}

impl ReceiverMiddleware {
  pub fn new<F>(f: F) -> Self
  where
    F: Fn(ReceiverMiddlewareChain) -> ReceiverMiddlewareChain + Send + Sync + 'static, {
    ReceiverMiddleware(Arc::new(f))
  }

  pub fn run(&self, next: ReceiverMiddlewareChain) -> ReceiverMiddlewareChain {
    (self.0)(next)
  }
}

impl Debug for ReceiverMiddleware {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ReceiverMiddleware")
  }
}

impl PartialEq for ReceiverMiddleware {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for ReceiverMiddleware {}

impl std::hash::Hash for ReceiverMiddleware {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ReceiverMiddlewareChain) -> ReceiverMiddlewareChain).hash(state);
  }
}
