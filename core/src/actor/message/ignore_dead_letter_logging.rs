use crate::actor::message::Message;
use nexus_actor_message_derive_rs::Message;
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreDeadLetterLogging;

impl Message for IgnoreDeadLetterLogging {
    fn eq_message(&self, other: &dyn Message) -> bool {
        other.as_any().is::<IgnoreDeadLetterLogging>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn message_type(&self) -> &'static str {
        "IgnoreDeadLetterLogging"
    }
}

impl IgnoreDeadLetterLogging {
  pub fn new() -> Self {
    Self {}
  }
}

impl Default for IgnoreDeadLetterLogging {
  fn default() -> Self {
    IgnoreDeadLetterLogging::new()
  }
}

static_assertions::assert_impl_all!(IgnoreDeadLetterLogging: Send, Sync);

impl Display for IgnoreDeadLetterLogging {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "IgnoreDeadLetterLogging")
  }
}
