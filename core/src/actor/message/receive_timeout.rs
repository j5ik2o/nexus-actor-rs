use std::any::Any;

use crate::actor::message::message::Message;

#[derive(Debug, Clone)]
pub struct ReceiveTimeout;

impl Message for ReceiveTimeout {
  fn eq_message(&self, other: &dyn Message) -> bool {
    other.as_any().is::<ReceiveTimeout>()
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}
