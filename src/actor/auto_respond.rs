use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

use crate::actor::context::context_handle::ContextHandle;
use crate::actor::message::message::Message;
use crate::actor::message::message_handle::MessageHandle;
use crate::actor::message::response::ResponseHandle;

#[derive(Debug, Clone)]
pub struct AutoRespond(MessageHandle);

impl AutoRespond {
  pub fn new(message: MessageHandle) -> Self {
    Self(message)
  }
}

impl Message for AutoRespond {
  fn eq_message(&self, other: &dyn Message) -> bool {
    self.0.eq_message(other)
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }
}

pub trait AutoResponsive: Debug + Send + Sync + 'static {
  fn get_auto_response(&self, context: ContextHandle) -> ResponseHandle;
}

impl AutoResponsive for AutoRespond {
  fn get_auto_response(&self, _: ContextHandle) -> ResponseHandle {
    ResponseHandle::new(self.0.clone())
  }
}

#[derive(Debug, Clone)]
pub struct AutoRespondHandle(Arc<dyn AutoResponsive>);

impl AutoRespondHandle {
  pub fn new(auto_respond: Arc<dyn AutoResponsive>) -> Self {
    Self(auto_respond)
  }
}

impl AutoResponsive for AutoRespondHandle {
  fn get_auto_response(&self, context: ContextHandle) -> ResponseHandle {
    self.0.get_auto_response(context)
  }
}

impl PartialEq for AutoRespondHandle {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl Eq for AutoRespondHandle {}

impl std::hash::Hash for AutoRespondHandle {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    Arc::as_ptr(&self.0).hash(state)
  }
}
