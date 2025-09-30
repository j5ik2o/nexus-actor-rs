use std::sync::Arc;

use parking_lot::Mutex;

use crate::actor::message::message_handle::MessageHandle;

#[derive(Debug, Clone)]
pub struct MessageHandles(Arc<Mutex<Vec<MessageHandle>>>);

impl MessageHandles {
  pub fn new(msgs: impl IntoIterator<Item = MessageHandle>) -> Self {
    Self(Arc::new(Mutex::new(msgs.into_iter().collect())))
  }

  pub fn push(&self, msg: MessageHandle) {
    self.0.lock().push(msg);
  }

  pub fn pop(&self) -> Option<MessageHandle> {
    self.0.lock().pop()
  }

  pub fn len(&self) -> usize {
    self.0.lock().len()
  }

  pub fn is_empty(&self) -> bool {
    self.0.lock().is_empty()
  }

  pub fn clear(&self) {
    self.0.lock().clear();
  }

  pub fn to_values(&self) -> Vec<MessageHandle> {
    self.0.lock().clone()
  }
}
