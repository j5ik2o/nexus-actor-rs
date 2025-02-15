//! Mailbox message types.

use nexus_actor_utils_rs::collections::Element;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub enum MailboxMessage {
  SuspendMailbox,
  ResumeMailbox,
}

impl Element for MailboxMessage {}
