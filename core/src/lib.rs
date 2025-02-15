//! Core module provides actor system functionality.

pub mod actor;
pub mod event_stream;
pub mod metrics;

// Re-exports
pub use self::{
  actor::{
    ActorContext, ActorError, Context, Message, MessageHandle, MessageHeaders, MessageOrEnvelope, MessagePart, Pid,
    Process, ProcessHandle, Props, ReceiverContext, ReceiverPart, RootContext, SenderContext, SenderPart,
    SpawnerContext, SpawnerPart, StopperPart, TypedContext, TypedMessageEnvelope,
  },
  event_stream::EventStream,
  metrics::MetricsProvider,
};
