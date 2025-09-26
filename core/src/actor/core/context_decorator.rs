use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use crate::actor::context::ContextSnapshot;

#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub struct ContextDecorator(Arc<dyn Fn(ContextSnapshot) -> ContextSnapshot + Send + Sync + 'static>);

unsafe impl Send for ContextDecorator {}
unsafe impl Sync for ContextDecorator {}

impl ContextDecorator {
  pub fn new(f: impl Fn(ContextSnapshot) -> ContextSnapshot + Send + Sync + 'static) -> Self {
    ContextDecorator(Arc::new(f))
  }

  pub fn run(&self, snapshot: ContextSnapshot) -> ContextSnapshot {
    (self.0)(snapshot)
  }

  pub(crate) fn into_inner(self) -> Arc<dyn Fn(ContextSnapshot) -> ContextSnapshot + Send + Sync + 'static> {
    self.0
  }
}

impl Debug for ContextDecorator {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "ContextDecorator")
  }
}

impl PartialEq for ContextDecorator {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for ContextDecorator {}

impl std::hash::Hash for ContextDecorator {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(ContextSnapshot) -> ContextSnapshot).hash(state);
  }
}

static_assertions::assert_impl_all!(ContextDecorator: Send, Sync);
