use std::any::Any;
use std::fmt::Debug;

pub trait Message: Any + Debug + Send + Sync {
  fn as_any(&self) -> &dyn Any;
  fn message_type(&self) -> &'static str;
  fn eq_message(&self, other: &dyn Message) -> bool;
  fn get_type_name(&self) -> String {
    std::any::type_name_of_val(self).to_string()
  }
}

pub mod auto_receive_message;
pub mod auto_respond;
pub mod continuation;
pub mod dead_letter_response;
pub mod failure;
pub mod ignore_dead_letter_logging;
pub mod json_serializer;
pub mod message_batch;
pub mod message_batch_test;
pub mod message_handle;
pub mod message_handles;
pub mod message_headers;
pub mod message_or_envelope;
pub mod message_or_envelope_test;
pub mod not_influence_receive_timeout;
pub mod proto_serializer;
pub mod readonly_message_headers;
pub mod receive_timeout;
pub mod response;
pub mod serialization;
pub mod system_message;
pub mod terminate_reason;
pub mod touched;
pub mod typed_message_or_envelope;

pub use self::{
  auto_receive_message::AutoReceiveMessage as _,
  auto_respond::{AutoRespond, AutoResponsive},
  continuation::Continuation as _,
  failure::Failure,
  ignore_dead_letter_logging::IgnoreDeadLetterLogging,
  json_serializer::JsonSerializer,
  message_batch::MessageBatch,
  message_handle::MessageHandle,
  message_handles::MessageHandles,
  message_headers::{MessageHeaders, EMPTY_MESSAGE_HEADER},
  message_or_envelope::{
    unwrap_envelope_header, unwrap_envelope_message, unwrap_envelope_sender, wrap_envelope, MessageEnvelope,
  },
  not_influence_receive_timeout::{NotInfluenceReceiveTimeout, NotInfluenceReceiveTimeoutHandle},
  proto_serializer::ProtoSerializer,
  readonly_message_headers::{ReadonlyMessageHeaders, ReadonlyMessageHeadersHandle},
  receive_timeout::ReceiveTimeout,
  response::{Response, ResponseHandle},
  serialization::{MessageSerializer, SerializationError},
  system_message::SystemMessage,
  terminate_reason::TerminateReason,
  touched::Touched,
  typed_message_or_envelope::TypedMessageOrEnvelope,
};
