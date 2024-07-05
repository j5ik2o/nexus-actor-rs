use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::actor::actor::receiver_middleware_chain_func::ReceiverMiddlewareChainFunc;

#[derive(Clone)]
pub struct ReceiverMiddlewareFunc(
  Arc<dyn Fn(ReceiverMiddlewareChainFunc) -> ReceiverMiddlewareChainFunc + Send + Sync>,
);

unsafe impl Send for ReceiverMiddlewareFunc {}
unsafe impl Sync for ReceiverMiddlewareFunc {}


impl ReceiverMiddlewareFunc {
  pub fn new(f: impl Fn(ReceiverMiddlewareChainFunc) -> ReceiverMiddlewareChainFunc + Send + Sync + 'static) -> Self {
    ReceiverMiddlewareFunc(Arc::new(f))
  }

  pub fn run(&self, next: ReceiverMiddlewareChainFunc) -> ReceiverMiddlewareChainFunc {
    (self.0)(next)
  }
}


impl Debug for ReceiverMiddlewareFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ReceiverMiddleware")
  }
}

impl PartialEq for ReceiverMiddlewareFunc {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for ReceiverMiddlewareFunc {}

impl std::hash::Hash for ReceiverMiddlewareFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ReceiverMiddlewareChainFunc) -> ReceiverMiddlewareChainFunc).hash(state);
  }
}
