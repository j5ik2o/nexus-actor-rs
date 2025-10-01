pub use nexus_actor_core_rs::actor::core_types::auto_receive::{AutoReceiveMessage, TerminatedMessage};
use nexus_actor_core_rs::TerminateReason;

use crate::generated::actor::{Pid, Terminated, TerminatedReason};
use std::convert::TryFrom;

impl From<Terminated> for TerminatedMessage {
  fn from(value: Terminated) -> Self {
    let who = value.who.map(|pid| pid.to_core());
    let reason_proto = TerminatedReason::try_from(value.why).unwrap_or(TerminatedReason::Stopped);
    let reason = match reason_proto {
      TerminatedReason::Stopped => TerminateReason::Stopped,
      TerminatedReason::AddressTerminated => TerminateReason::AddressTerminated,
      TerminatedReason::NotFound => TerminateReason::NotFound,
    };
    TerminatedMessage::new(who, reason)
  }
}

impl From<&Terminated> for TerminatedMessage {
  fn from(value: &Terminated) -> Self {
    value.clone().into()
  }
}

impl From<TerminatedMessage> for Terminated {
  fn from(value: TerminatedMessage) -> Self {
    let who = value.who.map(|pid| Pid::from_core(pid));
    let why = match value.reason {
      TerminateReason::Stopped => TerminatedReason::Stopped as i32,
      TerminateReason::AddressTerminated => TerminatedReason::AddressTerminated as i32,
      TerminateReason::NotFound => TerminatedReason::NotFound as i32,
    };
    Terminated { who, why }
  }
}

impl From<&TerminatedMessage> for Terminated {
  fn from(value: &TerminatedMessage) -> Self {
    value.clone().into()
  }
}

impl From<Terminated> for AutoReceiveMessage {
  fn from(value: Terminated) -> Self {
    AutoReceiveMessage::Terminated(value.into())
  }
}

impl From<&Terminated> for AutoReceiveMessage {
  fn from(value: &Terminated) -> Self {
    value.clone().into()
  }
}

pub trait AutoReceiveMessageExt {
  fn to_proto(&self) -> Option<Terminated>;
}

impl AutoReceiveMessageExt for AutoReceiveMessage {
  fn to_proto(&self) -> Option<Terminated> {
    match self {
      AutoReceiveMessage::Terminated(terminated) => Some(terminated.into()),
      _ => None,
    }
  }
}
