//! Actor module provides core actor system functionality.

// Core modules
pub mod actor_system;
pub mod config;
pub mod context;
pub mod dispatch;
pub mod event_stream;
pub mod guardian;
pub mod message;
pub mod metrics;
pub mod pid;
pub mod process;
pub mod process_registry;
pub mod supervisor;
pub mod typed_context;

// Re-exports
pub use self::{
    actor_system::*,
    config::*,
    context::*,
    dispatch::*,
    event_stream::*,
    guardian::*,
    message::*,
    metrics::*,
    pid::{ExtendedPid, Pid},
    process::{new_process_handle, Process},
    process_registry::*,
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

// Re-export process types
pub use process::{
    Process,
    ProcessHandle,
    new_process_handle,
    from_box_process,
    from_arc_process,
};

// Re-export error types
pub use error_reason::ErrorReason;
pub use context::SpawnError;
