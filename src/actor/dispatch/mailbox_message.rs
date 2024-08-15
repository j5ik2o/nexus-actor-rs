use nexus_acto_message_derive_rs::Message;
use crate::actor::message::Message;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub enum MailboxMessage {
  SuspendMailbox,
  ResumeMailbox,
}