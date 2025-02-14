use crate::actor::actor::ExtendedPid;
use crate::actor::message::message_headers::MessageHeaders;
use crate::actor::message::message_or_envelope::MessageEnvelope;
use crate::actor::message::Message;
use std::any::Any;

#[derive(Debug, Clone)]
pub struct TypedMessageOrEnvelope<T: Message> {
  underlying: MessageEnvelope,
  _phantom: std::marker::PhantomData<T>,
}

impl<M: Message> PartialEq for TypedMessageOrEnvelope<M> {
  fn eq(&self, other: &Self) -> bool {
    self.underlying == other.underlying
  }
}

impl<M: Message> Message for TypedMessageOrEnvelope<M> {
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other) = other.as_any().downcast_ref::<Self>() {
      self == other
    } else {
      false
    }
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn message_type(&self) -> &'static str {
    "TypedMessageOrEnvelope"
  }
}

impl<M: Message + Clone> TypedMessageOrEnvelope<M> {
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

impl<M: Message + Clone> From<MessageEnvelope> for TypedMessageOrEnvelope<M> {
  fn from(underlying: MessageEnvelope) -> Self {
    TypedMessageOrEnvelope::new(underlying)
  }
}

impl<M: Message> From<TypedMessageOrEnvelope<M>> for MessageEnvelope {
  fn from(typed: TypedMessageOrEnvelope<M>) -> Self {
    typed.underlying
  }
}
