use crate::actor::message::Message;
use crate::generated::actor::PoisonPill;
use std::any::Any;

impl Message for PoisonPill {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<PoisonPill>();
    match (self, msg) {
      (PoisonPill {}, Some(PoisonPill {})) => true,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
