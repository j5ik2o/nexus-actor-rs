use crate::actor::actor::Pid;
use crate::actor::message::message::Message;
use nexus_actor_message_derive_rs::Message;

#[derive(Debug, Clone, PartialEq, Message)]
pub struct DeadLetterResponse {
  pub target: Option<Pid>,
}

impl Eq for DeadLetterResponse {}
