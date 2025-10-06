use crate::actor::message::Message;
use nexus_message_derive_rs::Message;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub enum MailboxMessage {
  SuspendMailbox,
  ResumeMailbox,
}
