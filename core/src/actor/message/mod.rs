//! Message module provides message handling functionality.

mod auto_receive_message;
mod auto_respond;
mod continuation;
mod dead_letter_response;
mod failure;
mod ignore_dead_letter_logging;
mod message;
mod message_batch;
mod message_handle;
mod message_headers;
mod message_or_envelope;
mod readonly_message_headers;
mod receive_timeout;
mod response;
mod serialization;
mod system_message;
mod touched;
mod typed_message_or_envelope;

pub use self::{
    auto_receive_message::*,
    auto_respond::*,
    continuation::*,
    dead_letter_response::*,
    failure::*,
    ignore_dead_letter_logging::*,
    message::*,
    message_batch::*,
    message_handle::*,
    message_headers::*,
    message_or_envelope::*,
    readonly_message_headers::*,
    receive_timeout::*,
    response::*,
    serialization::*,
    system_message::*,
    touched::*,
    typed_message_or_envelope::*,
};
