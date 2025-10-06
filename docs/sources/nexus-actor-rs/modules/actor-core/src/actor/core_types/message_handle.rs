#![cfg(feature = "alloc")]

use alloc::sync::Arc;
use core::any::Any;
use core::fmt::{self, Debug, Display, Formatter};
use core::hash::{Hash, Hasher};

use super::message::Message;
use nexus_utils_core_rs::collections::{Element, PriorityMessage};

pub struct MessageHandle(Arc<dyn Message>);

impl MessageHandle {
  pub fn new_arc(msg: Arc<dyn Message>) -> Self {
    if msg.as_any().downcast_ref::<MessageHandle>().is_some() {
      panic!("MessageHandle can't be used as a message, {:?}", msg);
    }
    MessageHandle(msg)
  }

  pub fn new(msg: impl Message + 'static) -> Self {
    MessageHandle(Arc::new(msg))
  }

  pub fn to_typed<T: Clone + 'static>(&self) -> Option<T> {
    self.0.as_any().downcast_ref::<T>().cloned()
  }

  pub fn as_typed<T: 'static>(&self) -> Option<&T> {
    self.0.as_any().downcast_ref::<T>()
  }

  pub fn is_typed<T: 'static>(&self) -> bool {
    self.0.as_any().is::<T>()
  }

  pub fn inner(&self) -> &Arc<dyn Message> {
    &self.0
  }
}

impl Clone for MessageHandle {
  fn clone(&self) -> Self {
    MessageHandle(self.0.clone())
  }
}

impl Debug for MessageHandle {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "MessageHandle({:?})", self.0)
  }
}

impl Display for MessageHandle {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self.0)
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

  fn get_type_name(&self) -> alloc::string::String {
    self.0.get_type_name()
  }
}

impl PartialEq for MessageHandle {
  fn eq(&self, other: &Self) -> bool {
    self.0.eq_message(other.0.as_ref())
  }
}

impl Eq for MessageHandle {}

impl Hash for MessageHandle {
  fn hash<H: Hasher>(&self, state: &mut H) {
    let ptr = Arc::as_ptr(&self.0) as *const ();
    ptr.hash(state);
  }
}
