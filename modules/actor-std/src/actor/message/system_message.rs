use crate::generated::actor::{Pid, Terminated, TerminatedReason, Unwatch, Watch};
use nexus_actor_core_rs::actor::core_types::auto_receive::TerminatedMessage;
pub use nexus_actor_core_rs::actor::core_types::system_message::{SystemMessage, UnwatchMessage, WatchMessage};
use nexus_actor_core_rs::TerminateReason;

/// `WatchMessage` と ProtoBuf `Watch` 型の相互変換を支援するエクステンション。
pub trait WatchMessageExt {
  fn to_proto(&self) -> Watch;
}

impl WatchMessageExt for WatchMessage {
  fn to_proto(&self) -> Watch {
    Watch {
      watcher: Some(Pid::from_core(self.watcher().clone())),
    }
  }
}

pub trait WatchProtoExt {
  fn to_core(&self) -> Option<WatchMessage>;
}

impl WatchProtoExt for Watch {
  fn to_core(&self) -> Option<WatchMessage> {
    self.watcher.as_ref().map(|pid| WatchMessage::new(pid.to_core()))
  }
}

/// `UnwatchMessage` と ProtoBuf `Unwatch` 型の相互変換を支援するエクステンション。
pub trait UnwatchMessageExt {
  fn to_proto(&self) -> Unwatch;
}

impl UnwatchMessageExt for UnwatchMessage {
  fn to_proto(&self) -> Unwatch {
    Unwatch {
      watcher: Some(Pid::from_core(self.watcher().clone())),
    }
  }
}

pub trait UnwatchProtoExt {
  fn to_core(&self) -> Option<UnwatchMessage>;
}

impl UnwatchProtoExt for Unwatch {
  fn to_core(&self) -> Option<UnwatchMessage> {
    self.watcher.as_ref().map(|pid| UnwatchMessage::new(pid.to_core()))
  }
}

/// `TerminatedMessage` と ProtoBuf `Terminated` 型の相互変換を補助するエクステンション。
pub trait TerminatedMessageExt {
  fn to_proto(&self) -> Terminated;
  fn from_proto(proto: &Terminated) -> TerminatedMessage;
}

impl TerminatedMessageExt for TerminatedMessage {
  fn to_proto(&self) -> Terminated {
    let who = self.who.as_ref().cloned().map(Pid::from_core);
    let why = match self.reason {
      TerminateReason::Stopped => TerminatedReason::Stopped as i32,
      TerminateReason::AddressTerminated => TerminatedReason::AddressTerminated as i32,
      TerminateReason::NotFound => TerminatedReason::NotFound as i32,
    };
    Terminated { who, why }
  }

  fn from_proto(proto: &Terminated) -> TerminatedMessage {
    let who = proto.who.as_ref().map(Pid::to_core);
    let reason = TerminatedReason::try_from(proto.why).unwrap_or(TerminatedReason::Stopped);
    let reason = match reason {
      TerminatedReason::Stopped => TerminateReason::Stopped,
      TerminatedReason::AddressTerminated => TerminateReason::AddressTerminated,
      TerminatedReason::NotFound => TerminateReason::NotFound,
    };
    TerminatedMessage::new(who, reason)
  }
}

/// SystemMessage を ProtoBuf 互換の表現へ変換するための補助列挙。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemMessageProto {
  Restart,
  Start,
  Stop,
  Watch(Watch),
  Unwatch(Unwatch),
  Terminate(Terminated),
}

pub trait SystemMessageProtoExt {
  fn to_proto(&self) -> SystemMessageProto;
}

impl SystemMessageProtoExt for SystemMessage {
  fn to_proto(&self) -> SystemMessageProto {
    match self {
      SystemMessage::Restart => SystemMessageProto::Restart,
      SystemMessage::Start => SystemMessageProto::Start,
      SystemMessage::Stop => SystemMessageProto::Stop,
      SystemMessage::Watch(watch) => SystemMessageProto::Watch(watch.to_proto()),
      SystemMessage::Unwatch(unwatch) => SystemMessageProto::Unwatch(unwatch.to_proto()),
      SystemMessage::Terminate(terminated) => SystemMessageProto::Terminate(terminated.to_proto()),
    }
  }
}

pub trait SystemMessageFromProtoExt {
  fn to_system_message(&self) -> Option<SystemMessage>;
}

impl SystemMessageFromProtoExt for Watch {
  fn to_system_message(&self) -> Option<SystemMessage> {
    self.to_core().map(SystemMessage::Watch)
  }
}

impl SystemMessageFromProtoExt for Unwatch {
  fn to_system_message(&self) -> Option<SystemMessage> {
    self.to_core().map(SystemMessage::Unwatch)
  }
}

impl SystemMessageFromProtoExt for Terminated {
  fn to_system_message(&self) -> Option<SystemMessage> {
    Some(SystemMessage::Terminate(TerminatedMessage::from_proto(self)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use nexus_actor_core_rs::CorePid;

  #[test]
  fn watch_roundtrip() {
    let core_pid = CorePid::new("addr".to_string(), "id".to_string());
    let msg = WatchMessage::new(core_pid.clone());
    let proto = msg.to_proto();
    let restored = proto.to_core().expect("watcher should be present");
    assert_eq!(restored.watcher(), msg.watcher());
  }

  #[test]
  fn terminate_roundtrip() {
    let core_pid = CorePid::new("addr".to_string(), "id".to_string());
    let original = TerminatedMessage::new(Some(core_pid.clone()), TerminateReason::NotFound);
    let proto = original.to_proto();
    let restored = TerminatedMessage::from_proto(&proto);
    assert_eq!(restored.who, original.who);
    assert_eq!(restored.reason, original.reason);
  }
}
