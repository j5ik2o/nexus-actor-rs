//! Actor module provides core actor system functionality.

pub mod actor;
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
    actor::*,
    actor_system::*,
    config::*,
    context::*,
    dispatch::*,
    event_stream::*,
    guardian::*,
    message::*,
    metrics::*,
    pid::*,
    process::*,
    process_registry::*,
    supervisor::*,
    typed_context::*,
};
