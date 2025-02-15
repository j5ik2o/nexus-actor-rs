//! Actor module provides core actor system functionality.

pub mod actor;
pub mod actor_error;
pub mod actor_handle;
pub mod actor_ref;
pub mod context;
pub mod dispatch;
pub mod error_reason;
pub mod event_stream;
pub mod guardian;
pub mod lifecycle;
pub mod message;
pub mod metrics;
pub mod pid;
pub mod process;
pub mod process_registry;
pub mod props;
pub mod restart_statistics;
pub mod spawner;
pub mod supervisor;
pub mod types;

// Re-exports with explicit paths to avoid ambiguity
pub use self::{
  actor::Actor,
  actor_error::ActorError,
  actor_handle::ActorHandle,
  actor_ref::ActorRef,
  context::{
    ActorContext, Context, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart, RootContext,
    SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
  },
  error_reason::ErrorReason,
  lifecycle::Lifecycle,
  message::{Message, MessageHandle, MessageHeaders, MessageOrEnvelope, TypedMessageEnvelope},
  pid::Pid,
  process::{new_process_handle, Process, ProcessHandle},
  props::Props,
  restart_statistics::RestartStatistics,
  spawner::Spawner,
  supervisor::SupervisorStrategy,
  types::*,
};
