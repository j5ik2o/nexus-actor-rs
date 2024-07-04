use std::any::Any;

use crate::actor::message::message_handle::Message;

#[derive(Debug, Clone)]
pub enum MailboxMessage {
  SuspendMailbox,
  ResumeMailbox,
}

impl Message for MailboxMessage {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().is::<MailboxMessage>()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
