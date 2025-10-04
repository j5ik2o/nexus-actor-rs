use crate::actor::message::Message;
use crate::generated::actor::PoisonPill;
use std::any::Any;

impl Message for PoisonPill {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<PoisonPill>();
    matches!((self, msg), (PoisonPill {}, Some(PoisonPill {})))
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}
