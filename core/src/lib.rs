//! Core module provides the foundational actor system functionality.

pub mod actor;
pub mod event_stream;
pub mod metrics;

// Re-export key types and traits
pub use self::{
  actor::{
    ActorContext, ActorError, ActorSystem, Context, ExtensionPart, InfoPart, Message, MessageHandle, MessageHeaders,
    MessageOrEnvelope, MessagePart, Pid, Process, ProcessHandle, Props, ReceiverContext, ReceiverPart, RootContext,
    SenderContext, SenderPart, SpawnContext, SpawnerPart, StopperPart, TypedContext, TypedMessageEnvelope,
  },
  event_stream::EventStream,
  metrics::{ActorMetrics, ProtoMetrics},
};
