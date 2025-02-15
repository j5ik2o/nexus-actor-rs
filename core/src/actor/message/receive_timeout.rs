use crate::actor::message::Message;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq)]
pub struct ReceiveTimeout;

impl Message for ReceiveTimeout {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}
