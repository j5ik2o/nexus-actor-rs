use crate::event_stream::event_handler::EventHandler;
use crate::event_stream::predicate::Predicate;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Subscription {
  id: i32,
  pub(crate) handler: Arc<EventHandler>,
  pub(crate) predicate: Option<Predicate>,
  active: Arc<AtomicU32>,
}

impl Subscription {
  pub fn new(id: i32, handler: Arc<EventHandler>, predicate: Option<Predicate>) -> Self {
    Subscription {
      id,
      handler,
      predicate,
      active: Arc::new(AtomicU32::new(1)),
    }
  }

  pub fn activate(&self) -> bool {
    self
      .active
      .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
      .is_ok()
  }

  pub fn deactivate(&self) -> bool {
    self
      .active
      .compare_exchange(1, 0, Ordering::SeqCst, Ordering::SeqCst)
      .is_ok()
  }

  pub fn is_active(&self) -> bool {
    self.active.load(Ordering::SeqCst) == 1
  }
}

static_assertions::assert_impl_all!(Subscription: Send, Sync);

impl PartialEq for Subscription {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

impl Eq for Subscription {}
