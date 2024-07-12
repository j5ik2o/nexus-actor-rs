use crate::actor::message::message::Message;
use std::any::Any;

#[derive(Debug, Clone, PartialEq)]
pub struct PoisonPill;

impl Message for PoisonPill {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().is::<PoisonPill>()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
