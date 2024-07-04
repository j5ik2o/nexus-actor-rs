use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

use crate::actor::message::message_handle::Message;

pub trait Response: Message + Debug + Send + Sync + 'static {
  fn eq_response(&self, other: &dyn Response) -> bool;
}

#[derive(Debug, Clone)]
pub struct ResponseHandle(Arc<dyn Response>);

impl ResponseHandle {
  pub fn new(response: Arc<dyn Response>) -> Self {
    ResponseHandle(response)
  }
}

impl PartialEq for ResponseHandle {
  fn eq(&self, other: &Self) -> bool {
    self.0.eq_response(other.0.as_ref())
  }
}

impl Message for ResponseHandle {
  fn eq_message(&self, other: &dyn Message) -> bool {
    match (
      self.0.as_any().downcast_ref::<ResponseHandle>(),
      other.as_any().downcast_ref::<ResponseHandle>(),
    ) {
      (Some(self_msg), Some(other_msg)) => self_msg == other_msg,
      _ => false,
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self.0.as_any()
  }
}

impl Response for ResponseHandle {
  fn eq_response(&self, other: &dyn Response) -> bool {
    self.0.eq_response(other)
  }
}
