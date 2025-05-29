use crate::actor::message::MessageHandle;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

#[cfg(test)]
mod tests;

// Predicate is a function used to filter messages before being forwarded to a subscriber
#[derive(Clone)]
pub struct Predicate(Arc<dyn Fn(MessageHandle) -> bool + Send + Sync + 'static>);

unsafe impl Send for Predicate {}
unsafe impl Sync for Predicate {}

impl Predicate {
  pub fn new(f: impl Fn(MessageHandle) -> bool + Send + Sync + 'static) -> Self {
    Predicate(Arc::new(f))
  }

  pub fn run(&self, evt: MessageHandle) -> bool {
    (self.0)(evt)
  }
}

impl Debug for Predicate {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "Predicate")
  }
}

impl PartialEq for Predicate {
  fn eq(&self, _other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &_other.0)
  }
}

impl Eq for Predicate {}

impl std::hash::Hash for Predicate {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Fn(MessageHandle) -> bool).hash(state);
  }
}
