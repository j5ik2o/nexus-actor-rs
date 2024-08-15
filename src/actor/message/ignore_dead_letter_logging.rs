use crate::actor::message::message::Message;
use nexus_acto_message_derive_rs::Message;
use std::fmt::Display;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct IgnoreDeadLetterLogging;

impl IgnoreDeadLetterLogging {
  pub fn new() -> Self {
    Self {}
  }
}

static_assertions::assert_impl_all!(IgnoreDeadLetterLogging: Send, Sync);

impl Display for IgnoreDeadLetterLogging {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "IgnoreDeadLetterLogging")
  }
}
