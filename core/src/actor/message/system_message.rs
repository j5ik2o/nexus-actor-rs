use crate::actor::message::message::Message;
use crate::generated::actor::{Terminated, Unwatch, Watch};
use std::any::Any;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemMessage {
  Restart,
  Start,
  Stop,
  Watch(Watch),
  Unwatch(Unwatch),
  Terminate(Terminated),
}

impl SystemMessage {
  pub fn of_restart() -> Self {
    SystemMessage::Restart
  }

  pub fn of_start() -> Self {
    SystemMessage::Start
  }

  pub fn of_stop() -> Self {
    SystemMessage::Stop
  }

  pub fn of_watch(watch: Watch) -> Self {
    SystemMessage::Watch(watch)
  }

  pub fn of_unwatch(unwatch: Unwatch) -> Self {
    SystemMessage::Unwatch(unwatch)
  }

  pub fn of_terminate(terminate: Terminated) -> Self {
    SystemMessage::Terminate(terminate)
  }
}

impl Message for SystemMessage {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<SystemMessage>();
    match (self, msg) {
      (SystemMessage::Restart, Some(&SystemMessage::Restart)) => true,
      (SystemMessage::Start, Some(&SystemMessage::Start)) => true,
      (SystemMessage::Stop, Some(&SystemMessage::Stop)) => true,
      (SystemMessage::Watch(_), Some(&SystemMessage::Watch(_))) => true,
      (SystemMessage::Unwatch(_), Some(&SystemMessage::Unwatch(_))) => true,
      (SystemMessage::Terminate(me), Some(&SystemMessage::Terminate(ref you))) => *me == *you,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl SystemMessage {
  fn as_any(&self) -> &dyn Any {
    self
  }

  fn system_message(&self) {}
}
