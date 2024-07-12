use std::any::Any;
use std::fmt::Display;

use crate::actor::message::message::Message;

#[derive(Debug, Clone)]
pub struct IgnoreDeadLetterLogging {}

impl IgnoreDeadLetterLogging {
  pub fn new() -> Self {
    Self {}
  }
}

static_assertions::assert_impl_all!(IgnoreDeadLetterLogging: Send, Sync);

impl PartialEq for IgnoreDeadLetterLogging {
  fn eq(&self, other: &Self) -> bool {
    self.eq_message(other)
  }
}

impl Eq for IgnoreDeadLetterLogging {}

impl Display for IgnoreDeadLetterLogging {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "IgnoreDeadLetterLogging")
  }
}

impl Message for IgnoreDeadLetterLogging {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().is::<IgnoreDeadLetterLogging>()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
