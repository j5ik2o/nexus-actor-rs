//! Actor module provides core actor system functionality.

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

// Re-exports with explicit paths to avoid ambiguity
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
