use crate::actor::message::message::Message;
use nexus_acto_message_derive_rs::Message;

#[derive(Debug, Clone, PartialEq, Eq, Message)]
pub struct PoisonPill;
