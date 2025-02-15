use crate::actor::message::Message;
use crate::actor::ActorError;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone)]
pub struct Failure {
  pub who: Option<String>,
  pub error: ActorError,
}

impl Message for Failure {
  fn as_any(&self) -> &(dyn Any + Send + Sync) {
    self
  }
}

impl PartialEq for Failure {
  fn eq(&self, other: &Self) -> bool {
    self.who == other.who
  }
}
