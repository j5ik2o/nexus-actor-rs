//! Actor module provides core actor system functionality.

// Core modules
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

// System modules
pub mod actor_system;
pub mod config;
pub mod context;
pub mod dispatch;
pub mod event_stream;
pub mod guardian;
pub mod metrics;
pub mod process_registry;
pub mod supervisor;
pub mod typed_context;

// Re-exports with explicit paths to avoid ambiguity
pub use self::{
    actor_error::*,
    actor_handle::*,
    actor_ref::*,
    error_reason::ErrorReason,
    lifecycle::*,
    message::*,
    pid::{ExtendedPid, Pid},
    process::{new_process_handle, Process},
    props::*,
    restart_statistics::*,
    spawner::*,
    types::*,
    // System re-exports
    actor_system::*,
    config::*,
    context::*,
    dispatch::*,
    event_stream::*,
    guardian::*,
    metrics::*,
    process_registry::*,
    supervisor::*,
    typed_context::*,
};
