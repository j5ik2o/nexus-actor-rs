use std::any::Any;
use std::fmt::Debug;

pub trait Message: Any + Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn message_type(&self) -> &'static str;
}

mod auto_receive_message;
mod auto_respond;
mod continuation;
mod dead_letter_response;
mod failure;
mod ignore_dead_letter_logging;
mod json_serializer;
mod message_batch;
mod message_batch_test;
mod message_handle;
mod message_handles;
mod message_headers;
mod message_or_envelope;
mod message_or_envelope_test;
mod not_influence_receive_timeout;
mod proto_serializer;
mod readonly_message_headers;
mod receive_timeout;
mod response;
mod serialization;
mod system_message;
mod terminate_reason;
mod touched;
mod typed_message_or_envelope;

pub use self::{
    auto_receive_message::AutoReceiveMessage,
    auto_respond::AutoRespond,
    continuation::Continuation,
    failure::Failure,
    ignore_dead_letter_logging::IgnoreDeadLetterLogging,
    json_serializer::JsonSerializer,
    message_batch::MessageBatch,
    message_handle::MessageHandle,
    message_handles::MessageHandles,
    message_headers::MessageHeaders,
    message_or_envelope::MessageOrEnvelope,
    not_influence_receive_timeout::NotInfluenceReceiveTimeout,
    proto_serializer::ProtoSerializer,
    readonly_message_headers::ReadonlyMessageHeaders,
    receive_timeout::ReceiveTimeout,
    response::Response,
    serialization::{MessageSerializer, SerializationError},
    system_message::SystemMessage,
    terminate_reason::TerminateReason,
    touched::Touched,
    typed_message_or_envelope::TypedMessageOrEnvelope,
};
