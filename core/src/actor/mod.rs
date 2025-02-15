//! Actor module provides core actor system functionality.

// Core modules
pub mod actor;
pub mod actor_error;
pub mod actor_handle;
pub mod actor_inner_error;
pub mod actor_process;
pub mod actor_producer;
pub mod actor_receiver;
pub mod actor_ref;
pub mod config;
pub mod context;
pub mod context_decorator;
pub mod context_decorator_chain;
pub mod context_handler;
pub mod continuer;
pub mod dispatch;
pub mod error_reason;
pub mod event_stream;
pub mod guardian;
pub mod lifecycle;
pub mod message;
pub mod metrics;
pub mod middleware;
pub mod middleware_chain;
pub mod pid;
pub mod process;
pub mod process_registry;
pub mod props;
pub mod receiver_middleware;
pub mod receiver_middleware_chain;
pub mod restart_statistics;
pub mod sender_middleware;
pub mod sender_middleware_chain;
pub mod spawn_middleware;
pub mod supervisor;
pub mod typed_actor;
pub mod typed_actor_handle;
pub mod typed_actor_producer;
pub mod typed_actor_receiver;
pub mod typed_context;
pub mod typed_pid;
pub mod typed_props;
pub mod types;

// Re-exports
pub use self::{
    actor::*,
    actor_error::*,
    actor_handle::*,
    actor_inner_error::*,
    actor_process::*,
    actor_producer::*,
    actor_receiver::*,
    actor_ref::*,
    config::*,
    context::*,
    context_decorator::*,
    context_decorator_chain::*,
    context_handler::*,
    continuer::*,
    dispatch::*,
    error_reason::ErrorReason,
    event_stream::*,
    guardian::*,
    lifecycle::*,
    message::*,
    metrics::*,
    middleware::*,
    middleware_chain::*,
    pid::{ExtendedPid, Pid},
    process::{new_process_handle, Process},
    process_registry::*,
    props::*,
    receiver_middleware::*,
    receiver_middleware_chain::*,
    restart_statistics::*,
    sender_middleware::*,
    sender_middleware_chain::*,
    spawn_middleware::*,
    supervisor::*,
    typed_actor::*,
    typed_actor_handle::*,
    typed_actor_producer::*,
    typed_actor_receiver::*,
    typed_context::*,
    typed_pid::*,
    typed_props::*,
    types::*,
};

// Re-export generated types
pub use crate::generated::actor::Pid as GeneratedPid;
