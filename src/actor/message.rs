use crate::core::actor::actor_ref::{InternalActorRef, LocalActorRef};
use crate::core::util::element::Element;

pub trait Message: Element + 'static {}

#[derive(Debug, Clone)]
pub enum AutoReceivedMessage {
  PoisonPill,
  Terminated {
    actor: InternalActorRef,
    existence_confirmed: bool,
    address_terminated: bool,
  },
}

impl AutoReceivedMessage {
  pub fn terminated(actor: InternalActorRef, existence_confirmed: bool, address_terminated: bool) -> Self {
    Self::Terminated {
      actor,
      existence_confirmed,
      address_terminated,
    }
  }
}

impl Element for AutoReceivedMessage {}
impl Message for AutoReceivedMessage {}
