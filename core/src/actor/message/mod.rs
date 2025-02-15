//! Message module provides message-related functionality.

use std::any::Any;
use std::fmt::Debug;

pub trait Message: Debug + Send + Sync + 'static {
  fn as_any(&self) -> &(dyn Any + Send + Sync);
  fn message_type(&self) -> &'static str {
    std::any::type_name::<Self>()
  }
  fn eq_message(&self, other: &dyn Message) -> bool {
    if let Some(other) = other.as_any().downcast_ref::<Self>() {
      self == other
    } else {
      false
    }
  }
}

pub mod auto_receive_message;
pub mod auto_respond;
pub mod continuation;
pub mod dead_letter_response;
pub mod failure;
pub mod ignore_dead_letter_logging;
pub mod message_batch;
pub mod message_handle;
pub mod message_headers;
pub mod message_or_envelope;
pub mod response;
pub mod system_message;
pub mod touched;
pub mod typed_message_or_envelope;

pub use self::{
  auto_receive_message::AutoReceiveMessage,
  auto_respond::AutoRespond,
  continuation::Continuation,
  dead_letter_response::DeadLetterResponse,
  failure::Failure,
  ignore_dead_letter_logging::IgnoreDeadLetterLogging,
  message_batch::MessageBatch,
  message_handle::MessageHandle,
  message_headers::MessageHeaders,
  message_or_envelope::MessageOrEnvelope,
  response::Response,
  system_message::SystemMessage,
  touched::Touched,
  typed_message_or_envelope::{TypedMessageEnvelope, TypedMessageOrEnvelope},
};
