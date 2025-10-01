use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::actor::context::ContextHandle;

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct ContextHandler(Arc<dyn Fn(ContextHandle) + Send + Sync + 'static>);

unsafe impl Send for ContextHandler {}
unsafe impl Sync for ContextHandler {}

impl ContextHandler {
  pub fn new(f: impl Fn(ContextHandle) + Send + Sync + 'static) -> Self {
    ContextHandler(Arc::new(f))
  }

  pub fn run(&self, ctx: ContextHandle) {
    self.0(ctx)
  }
}

impl Debug for ContextHandler {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContextHandler")
  }
}

impl PartialEq for ContextHandler {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for ContextHandler {}

impl std::hash::Hash for ContextHandler {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextHandle)).hash(state);
  }
}

static_assertions::assert_impl_all!(ContextHandler: Send, Sync);
