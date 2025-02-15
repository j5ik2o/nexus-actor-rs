//! Actor module provides core actor system functionality.

pub mod actor;
pub mod actor_system;
pub mod config;
pub mod context;
pub mod dispatch;
pub mod error_reason;
pub mod event_stream;
pub mod guardian;
pub mod message;
pub mod metrics;
pub mod pid;
pub mod process;
pub mod process_registry;
pub mod restart_statistics;
pub mod supervisor;
pub mod typed_context;

// Re-exports
pub use self::{
  actor::*,
  actor_system::ActorSystem,
  config::Config,
  context::{ActorContext, ActorContextImpl, ActorError, ActorHandle, Props, SpawnError},
  dispatch::*,
  error_reason::ErrorReason,
  event_stream::*,
  guardian::*,
  message::*,
  metrics::*,
  pid::{ExtendedPid, Pid},
  process::{Process, ProcessHandle},
  process_registry::*,
  restart_statistics::RestartStatistics,
  supervisor::*,
  typed_context::*,
};

// Re-export message types
pub use message::{Message, MessageHandle, MessageHeaders, MessageOrEnvelope, SystemMessage, TypedMessageOrEnvelope};

// Re-export process types
pub use process::{new_process_handle, ActorProcess};

// Re-export error types
pub use error_reason::ErrorReason as ActorError;
