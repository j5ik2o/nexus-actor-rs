use crate::actor::core::ExtendedPid;
use crate::actor::message::{Message, MessageEnvelope, MessageHeaders};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct TypedMessageEnvelope<T: Message> {
  underlying: MessageEnvelope,
  _phantom: std::marker::PhantomData<T>,
}

impl<M: Message> PartialEq for TypedMessageEnvelope<M> {
  fn eq(&self, other: &Self) -> bool {
    self.underlying == other.underlying
  }
}

impl<M: Message> Message for TypedMessageEnvelope<M> {
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other) = other.as_any().downcast_ref::<Self>() {
      self == other
    } else {
      false
    }
  }

  fn as_any(&self) -> &(dyn Any + Send + Sync + 'static) {
    self
  }

  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

impl<M: Message + Clone> TypedMessageEnvelope<M> {
  pub fn new(underlying: MessageEnvelope) -> Self {
    Self {
      underlying,
      _phantom: std::marker::PhantomData,
    }
  }

  pub fn get_message(&self) -> M {
    self.underlying.get_message_handle().to_typed::<M>().unwrap()
  }

  pub fn get_sender(&self) -> Option<ExtendedPid> {
    self.underlying.get_sender().clone()
  }

  pub fn get_header(&self) -> Option<MessageHeaders> {
    self.underlying.get_header().clone()
  }

  pub fn get_header_value(&self, key: &str) -> Option<String> {
    self.underlying.get_header_value(key)
  }

  pub fn set_header(&mut self, key: String, value: String) {
    self.underlying.set_header(key, value);
  }

  pub fn get_headers(&self) -> Option<MessageHeaders> {
    self.underlying.get_headers()
  }
}

impl<M: Message + Clone> From<MessageEnvelope> for TypedMessageEnvelope<M> {
  fn from(underlying: MessageEnvelope) -> Self {
    TypedMessageEnvelope::new(underlying)
  }
}

impl<M: Message> From<TypedMessageEnvelope<M>> for MessageEnvelope {
  fn from(typed: TypedMessageEnvelope<M>) -> Self {
    typed.underlying
  }
}
