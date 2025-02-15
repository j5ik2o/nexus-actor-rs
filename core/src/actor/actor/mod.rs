//! Actor module provides core actor system functionality.

pub mod actor;
pub mod actor_error;
pub mod actor_handle;
pub mod actor_ref;
pub mod error_reason;
pub mod lifecycle;
pub mod pid;
pub mod process;
pub mod props;
pub mod restart_statistics;
pub mod spawner;
pub mod types;

// Re-exports with explicit paths to avoid ambiguity
pub use self::actor::*;
pub use self::actor_error::*;
pub use self::actor_handle::*;
pub use self::actor_ref::*;
pub use self::error_reason::ErrorReason;
pub use self::lifecycle::*;
pub use self::pid::Pid;
pub use self::process::Process;
pub use self::props::*;
pub use self::restart_statistics::*;
pub use self::spawner::*;
pub use self::types::*;
