//! Nexus Actor Core - A Rust implementation of the Proto.Actor framework

pub mod actor;
pub mod event_stream;

// Re-exports
pub use actor::{
  actor_error::ActorError,
  actor_handle::ActorHandle,
  actor_ref::ActorRef,
  context::actor_context::{
    ActorSystem, Context, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart, RootContext,
    SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart, TypedContext, TypedRootContext,
  },
  error_reason::ErrorReason,
  event_stream::EventStream,
  message::*,
  pid::Pid,
  process::{Process, ProcessHandle},
  props::Props,
  restart_statistics::*,
  spawner::*,
  supervisor::*,
};

pub use event_stream::*;
