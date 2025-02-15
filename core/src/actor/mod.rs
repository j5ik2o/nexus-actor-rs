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

// Re-exports with explicit paths to avoid ambiguity
pub use self::{
  actor::*,
  actor_error::*,
  actor_handle::*,
  actor_ref::*,
  context::*,
  dispatch::*,
  error_reason::ErrorReason,
  event_stream::*,
  guardian::*,
  lifecycle::*,
  message::*,
  metrics::*,
  pid::{ExtendedPid, Pid},
  process::{new_process_handle, Process},
  process_registry::*,
  props::*,
  restart_statistics::*,
  spawner::*,
  supervisor::*,
  typed_context::*,
};
