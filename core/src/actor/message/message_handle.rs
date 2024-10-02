use crate::actor::message::message::Message;
use nexus_actor_utils_rs::collections::{Element, PriorityMessage};
use std::any::Any;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;

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

  pub fn to_typed<T: Clone + 'static>(&self) -> Option<T> {
    if let Some(msg) = self.0.as_any().downcast_ref::<T>() {
      Some(msg.clone())
    } else {
      None
    }
  }

  pub fn as_typed<T: 'static>(&self) -> Option<&T> {
    self.0.as_any().downcast_ref::<T>()
  }

  pub fn is_typed<T: 'static>(&self) -> bool {
    self.0.as_any().is::<T>()
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

  fn get_type_name(&self) -> String {
    self.0.get_type_name()
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
    write!(f, "{:?}", self.0)
  }
}
