//! Actor module provides core actor system functionality.

pub mod actor;
pub mod actor_error;
pub mod actor_handle;
pub mod actor_ref;
pub mod error_reason;
pub mod lifecycle;
pub mod message;
pub mod pid;
pub mod process;
pub mod props;
pub mod restart_statistics;
pub mod spawner;
pub mod types;

// Re-exports with explicit paths to avoid ambiguity
pub use self::{
  actor::Actor,
  actor_error::ActorError,
  actor_handle::ActorHandle,
  actor_ref::ActorRef,
  error_reason::ErrorReason,
  lifecycle::Lifecycle,
  message::Message,
  pid::Pid,
  process::{new_process_handle, Process},
  props::Props,
  restart_statistics::RestartStatistics,
  spawner::Spawner,
  types::*,
};
