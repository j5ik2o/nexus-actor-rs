//! Message module provides message-related functionality.

pub mod auto_receive_message;
pub mod auto_respond;
pub mod continuation;
pub mod dead_letter_response;
pub mod failure;
pub mod ignore_dead_letter_logging;
pub mod json_serializer;
pub mod message;
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
  message::Message, message_handle::MessageHandle, message_or_envelope::MessageOrEnvelope,
  typed_message_or_envelope::TypedMessageOrEnvelope,
};

pub type TypedMessageEnvelope<T> = TypedMessageOrEnvelope<T>;
