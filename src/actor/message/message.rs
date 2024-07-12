use std::any::Any;
use std::fmt::Debug;

use crate::util::queue::priority_queue::DEFAULT_PRIORITY;

pub trait Message: Debug + Send + Sync + 'static {
  fn get_priority(&self) -> i8 {
    DEFAULT_PRIORITY
  }
  fn eq_message(&self, other: &dyn Message) -> bool;
  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static);
}

impl<T: prost::Message + PartialEq + 'static> Message for T {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match (self.as_any().downcast_ref::<T>(), other.as_any().downcast_ref::<T>()) {
      (Some(self_msg), Some(other_msg)) => self_msg == other_msg,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}
