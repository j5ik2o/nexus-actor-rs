//! Core module provides the foundational actor system functionality.

pub mod actor;
pub mod event_stream;
pub mod metrics;

pub use self::{
  actor::{
    context::{
      ActorContext, Context, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart, RootContext,
      SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart, TypedContext,
    },
    message::{Message, MessageHandle, MessageHeaders, MessageOrEnvelope, TypedMessageEnvelope},
    process::{Process, ProcessHandle},
    ActorError, ActorHandle, ActorSystem, ErrorReason, ExtendedPid, Pid, Props, SpawnError,
  },
  event_stream::EventStream,
  metrics::{ActorMetrics, ProtoMetrics},
};
