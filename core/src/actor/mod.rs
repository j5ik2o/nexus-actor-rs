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
    actor_system::*,
    config::*,
    context::*,
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

// Re-export message traits and types
pub use message::{
    Message,
    MessageHandle,
    MessageHeaders,
    SystemMessage,
    UserMessage,
    TypedMessageOrEnvelope,
    AutoReceiveMessage,
    DeadLetterResponse,
    Failure,
    ReceiveTimeout,
    Touched,
    IgnoreDeadLetterLogging,
    MessageBatch,
    AutoRespond,
    Continuation,
};

// Re-export error types
pub use error_reason::ErrorReason;
pub use context::{
    ActorError,
    ActorHandle,
    Continuer,
    Props,
    SpawnError,
};
