#![cfg(feature = "alloc")]

use alloc::string::String;
use alloc::sync::Arc;
use core::any::Any;
use core::fmt::Debug;

use super::message::Message;

pub trait Response: Message + Debug + Send + Sync + 'static {
  fn eq_response(&self, other: &dyn Response) -> bool;
}

impl<T> Response for T
where
  T: Message + Debug + Send + Sync + 'static,
{
  fn eq_response(&self, other: &dyn Response) -> bool {
    other.eq_message(self)
  }
}

#[derive(Debug, Clone)]
pub struct ResponseHandle(Arc<dyn Response>);

impl ResponseHandle {
  pub fn new_arc(response: Arc<dyn Response>) -> Self {
    ResponseHandle(response)
  }

  pub fn new(response: impl Response + 'static) -> Self {
    ResponseHandle(Arc::new(response))
  }

  pub fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self.0.as_any()
  }

  pub fn get_type_name(&self) -> String {
    self.0.get_type_name()
  }
}

impl PartialEq for ResponseHandle {
  fn eq(&self, other: &Self) -> bool {
    self.0.eq_response(other.0.as_ref())
  }
}

impl Eq for ResponseHandle {}

impl Message for ResponseHandle {
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other_handle) = other.as_any().downcast_ref::<ResponseHandle>() {
      self == other_handle
    } else {
      false
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self.0.as_any()
  }

  fn get_type_name(&self) -> String {
    self.0.get_type_name()
  }
}
