//! Message module.

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
  auto_receive_message::*, auto_respond::*, continuation::*, dead_letter_response::*, failure::*,
  ignore_dead_letter_logging::*, json_serializer::*, message::*, message_batch::*, message_handle::*,
  message_headers::*, message_or_envelope::*, proto_serializer::*, receive_timeout::*, response::*, serialization::*,
  system_message::*, touched::*, typed_message_or_envelope::*,
};

pub type TypedMessageEnvelope<T> = typed_message_or_envelope::TypedMessageOrEnvelope<T>;
