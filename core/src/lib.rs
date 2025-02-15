//! Nexus Actor Core - A Rust implementation of the Proto.Actor framework

pub mod actor;
pub mod event_stream;

// Re-exports
pub use actor::{
  actor_error::ActorError,
  actor_handle::ActorHandle,
  actor_ref::ActorRef,
  context::actor_context::{
    Context, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart, RootContext, SenderContext,
    SenderPart, SpawnerContext, SpawnerPart, StopperPart,
  },
  error_reason::ErrorReason,
  event_stream::EventStream,
  message::Message,
  pid::Pid,
  process::{Process, ProcessHandle},
  props::Props,
  restart_statistics::RestartStatistics,
  spawner::Spawner,
  supervisor::SupervisorStrategy,
  typed_context::TypedContext,
};

pub use event_stream::*;
