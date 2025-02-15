//! Message module provides message types and traits.

pub mod auto_receive_message;
pub mod auto_respond;
pub mod continuation;
pub mod dead_letter_response;
pub mod failure;
pub mod ignore_dead_letter_logging;
pub mod json_serializer;
pub mod message_batch;
pub mod message_handle;
pub mod message_headers;
pub mod message_or_envelope;
pub mod proto_serializer;
pub mod receive_timeout;
pub mod response;
pub mod serialization;
pub mod system_message;
pub mod touched;
pub mod typed_message_or_envelope;

pub use self::{
  auto_receive_message::AutoReceiveMessage, auto_respond::AutoRespond, continuation::Continuation,
  dead_letter_response::DeadLetterResponse, failure::Failure, ignore_dead_letter_logging::IgnoreDeadLetterLogging,
  message_batch::MessageBatch, message_handle::MessageHandle, message_headers::MessageHeaders,
  message_or_envelope::MessageOrEnvelope, receive_timeout::ReceiveTimeout, response::Response,
  system_message::SystemMessage, touched::Touched, typed_message_or_envelope::TypedMessageOrEnvelope,
};

use std::any::Any;
use std::fmt::Debug;

pub trait Message: Debug + Send + Sync + 'static {
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other) = other.as_any().downcast_ref::<Self>() {
      self == other
    } else {
      false
    }
  }

  fn as_any(&self) -> &dyn Any;
  fn message_type(&self) -> &'static str {
    std::any::type_name::<Self>()
  }
}

// Implement Message for all types that satisfy the trait bounds
impl<T: Debug + Send + Sync + 'static + PartialEq> Message for T {
  fn as_any(&self) -> &dyn Any {
    self
  }
}
