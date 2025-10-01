use crate::generated::remote::{ActorPidRequest, ActorPidResponse};
use nexus_actor_std_rs::actor::message::Message;
use std::any::Any;

impl Message for ActorPidRequest {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<ActorPidRequest>() {
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

impl Message for ActorPidResponse {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match other.as_any().downcast_ref::<ActorPidResponse>() {
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
