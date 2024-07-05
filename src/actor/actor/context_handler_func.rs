use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::actor::context::context_handle::ContextHandle;

#[derive(Clone)]
pub struct ContextHandleFunc(Arc<dyn Fn(ContextHandle) + Send + Sync>);

unsafe impl Send for ContextHandleFunc {}
unsafe impl Sync for ContextHandleFunc {}

impl ContextHandleFunc {
  pub fn new(f: impl Fn(ContextHandle) + Send + Sync + 'static) -> Self {
    ContextHandleFunc(Arc::new(f))
  }

  pub fn run(&self, ctx: ContextHandle) {
    self.0(ctx)
  }
}

impl Debug for ContextHandleFunc {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContextHandleFunc")
  }
}

impl PartialEq for ContextHandleFunc {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for ContextHandleFunc {}

impl std::hash::Hash for ContextHandleFunc {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle)).hash(state);
  }
}

