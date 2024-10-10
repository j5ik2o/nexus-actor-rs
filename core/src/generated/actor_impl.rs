use std::any::Any;
use std::hash::Hash;
use crate::actor::message::Message;
use crate::generated::actor::{DeadLetterResponse, Pid, PoisonPill, Terminated, Unwatch, Watch};

impl Pid {
  pub fn new(address: &str, id: &str) -> Self {
    Pid {
      address: address.to_string(),
      id: id.to_string(),
      request_id: 0,
    }
  }

  pub(crate) fn with_request_id(mut self, request_id: u32) -> Self {
    self.request_id = request_id;
    self
  }
}

impl std::fmt::Display for Pid {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}-{}-{}", self.address, self.id, self.request_id)
  }
}

impl Hash for Pid {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.address.hash(state);
    self.id.hash(state);
    self.request_id.hash(state);
  }
}

impl Eq for Pid {}

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

impl Eq for Watch {}

impl Eq for Unwatch {}

impl Eq for Terminated {}

impl Eq for DeadLetterResponse {}
