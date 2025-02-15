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
pub mod typed_context;

// Re-exports
pub use self::{
  actor::*,
  actor_error::ActorError,
  actor_handle::ActorHandle,
  actor_ref::ActorRef,
  context::{
    actor_context::ActorContext, actor_context::Context, actor_context::ExtensionPart, actor_context::InfoPart,
    actor_context::MessagePart, actor_context::ReceiverContext, actor_context::ReceiverPart,
    actor_context::RootContext, actor_context::SenderContext, actor_context::SenderPart, actor_context::SpawnerContext,
    actor_context::SpawnerPart, actor_context::StopperPart, actor_context::TypedContext,
    actor_context::TypedRootContext,
  },
  dispatch::*,
  error_reason::ErrorReason,
  event_stream::EventStream,
  guardian::*,
  lifecycle::*,
  message::*,
  metrics::*,
  pid::Pid,
  process::{Process, ProcessHandle},
  process_registry::*,
  props::Props,
  restart_statistics::*,
  spawner::*,
  supervisor::*,
  typed_context::*,
};

// Type aliases
pub type SpawnError = Box<dyn std::error::Error + Send + Sync>;
