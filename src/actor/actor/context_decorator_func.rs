use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::actor::actor::context_decorator_chain_func::ContextDecoratorChainFunc;

#[derive(Clone)]
pub struct ContextDecoratorFunc(Arc<dyn Fn(ContextDecoratorChainFunc) -> ContextDecoratorChainFunc + Send + Sync>);

impl Debug for ContextDecoratorFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContextDecorator")
  }
}

impl PartialEq for ContextDecoratorFunc {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for ContextDecoratorFunc {}

impl std::hash::Hash for ContextDecoratorFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextDecoratorChainFunc) -> ContextDecoratorChainFunc).hash(state);
  }
}

impl ContextDecoratorFunc {
  pub fn new(f: impl Fn(ContextDecoratorChainFunc) -> ContextDecoratorChainFunc + Send + Sync + 'static) -> Self {
    ContextDecoratorFunc(Arc::new(f))
  }

  pub fn run(&self, next: ContextDecoratorChainFunc) -> ContextDecoratorChainFunc {
    (self.0)(next)
  }
}
