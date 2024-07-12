use crate::actor::actor::Pid;
use crate::actor::message::message::Message;
use std::any::Any;

#[derive(Debug, Clone, PartialEq)]
pub struct DeadLetterResponse {
  pub target: Option<Pid>,
}

impl Message for DeadLetterResponse {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<DeadLetterResponse>() {
      Some(a) => self == a,
      None => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
