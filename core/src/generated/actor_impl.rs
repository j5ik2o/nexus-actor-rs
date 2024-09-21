use crate::actor::message::Message;
use crate::generated::actor::{Stop, Terminated, Unwatch, Watch};
use std::any::Any;

impl Message for Stop {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<Stop>();
    match (self, msg) {
      (Stop {}, Some(&Stop {})) => true,
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

impl Message for Terminated {
  fn eq_message(&self, other: &dyn Message) -> bool {
    let msg = other.as_any().downcast_ref::<Terminated>();
    match (self, msg) {
      (l, Some(r)) => *l == *r,
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

impl Eq for Watch {}

impl Eq for Unwatch {}

impl Eq for Terminated {}