//! Actor module provides core actor system functionality.

// Core modules
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
    actor_system::*,
    config::*,
    context::*,
    dispatch::*,
    error_reason::ErrorReason,
    event_stream::*,
    guardian::*,
    message::*,
    metrics::*,
    pid::{ExtendedPid, Pid, PidExt},
    process::{Process, ProcessHandle, new_process_handle},
    process_registry::*,
    restart_statistics::RestartStatistics,
    supervisor::*,
    typed_context::*,
};

// Re-export generated types
pub use crate::generated::actor::Pid as GeneratedPid;

// Re-export message traits and types
pub use message::{
    Message,
    MessageHandle,
    MessageHeaders,
    SystemMessage,
    UserMessage,
    TypedMessageOrEnvelope,
};
