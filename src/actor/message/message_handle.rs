use std::any::Any;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;

use crate::util::element::Element;
use crate::util::queue::priority_queue::{PriorityMessage, DEFAULT_PRIORITY};

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

#[derive(Debug, Clone)]
pub struct MessageHandle(Arc<dyn Message>);

impl MessageHandle {
  pub fn new_arc(msg: Arc<dyn Message>) -> Self {
    if msg.as_any().downcast_ref::<MessageHandle>().is_some() {
      panic!("MessageHandle can't be used as a message, {:?}", msg);
    }
    MessageHandle(msg)
  }

  pub fn new(msg: impl Message + Send + Sync + 'static) -> Self {
    MessageHandle(Arc::new(msg))
  }
}

impl Element for MessageHandle {}

impl PriorityMessage for MessageHandle {
  fn get_priority(&self) -> Option<i8> {
    Some(self.0.get_priority())
  }
}

impl Message for MessageHandle {
  fn eq_message(&self, other: &dyn Message) -> bool {
    self.0.eq_message(other)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self.0.as_any()
  }
}

impl PartialEq for MessageHandle {
  fn eq(&self, other: &Self) -> bool {
    self.0.eq_message(&*other.0)
  }
}

impl Eq for MessageHandle {}

impl std::hash::Hash for MessageHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    (self.0.as_ref() as *const dyn Message).hash(state);
  }
}

impl Display for MessageHandle {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.to_string())
  }
}