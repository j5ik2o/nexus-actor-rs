use crate::actor::actor_system::ActorSystem;
use crate::actor::core::ExtendedPid;
use crate::actor::message::{Message, MessageHandle};
use std::fmt::Display;

#[derive(Debug, Clone)]
pub struct TypedExtendedPid<M: Message> {
  underlying: ExtendedPid,
  _phantom: std::marker::PhantomData<M>,
}

impl<M: Message> PartialEq for TypedExtendedPid<M> {
  fn eq(&self, other: &Self) -> bool {
    self.underlying == other.underlying
  }
}

impl<M: Message> Eq for TypedExtendedPid<M> {}

impl<M: Message> Display for TypedExtendedPid<M> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.underlying)
  }
}

impl<M: Message> TypedExtendedPid<M> {
  pub fn new(underlying: ExtendedPid) -> Self {
    Self {
      underlying,
      _phantom: std::marker::PhantomData,
    }
  }

  pub(crate) fn get_underlying(&self) -> &ExtendedPid {
    &self.underlying
  }

  pub async fn send_user_message(&self, actor_system: ActorSystem, message: M) {
    self
      .underlying
      .send_user_message(actor_system, MessageHandle::new(message))
      .await
  }

  pub async fn send_system_message(&self, actor_system: ActorSystem, message: M) {
    self
      .underlying
      .send_system_message(actor_system, MessageHandle::new(message))
      .await
  }
}

impl<M: Message> From<ExtendedPid> for TypedExtendedPid<M> {
  fn from(underlying: ExtendedPid) -> Self {
    Self::new(underlying)
  }
}

impl<M: Message> From<TypedExtendedPid<M>> for ExtendedPid {
  fn from(typed: TypedExtendedPid<M>) -> Self {
    typed.underlying
  }
}
