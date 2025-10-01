use crate::actor::message::message_base::Message;
use crate::generated::actor::Pid;
use std::any::Any;

#[derive(Debug, Clone, PartialEq)]
pub struct Touched {
  pub who: Option<Pid>,
}

impl Message for Touched {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<Touched>() {
      Some(a) => self == a,
      None => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}
