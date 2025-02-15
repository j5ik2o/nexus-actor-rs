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
pub mod system;
pub mod types;

// Re-exports with explicit paths to avoid ambiguity
pub use self::actor::Actor;
pub use self::actor_error::ActorError;
pub use self::actor_handle::ActorHandle;
pub use self::actor_ref::ActorRef;
pub use self::context::{
  actor_context::{ActorContext, Context, InfoPart, MessagePart, ReceiverPart, SenderPart, SpawnerPart, StopperPart},
  mock_context::MockContext,
  root_context::RootContext,
  typed_actor_context::TypedActorContext,
  typed_context_handle::TypedContextHandle,
  typed_root_context::TypedRootContext,
};
pub use self::error_reason::ErrorReason;
pub use self::event_stream::EventStream;
pub use self::guardian::GuardianProcess;
pub use self::lifecycle::Lifecycle;
pub use self::message::{Message, MessageHandle, MessageOrEnvelope};
pub use self::metrics::MetricsProvider;
pub use self::pid::Pid;
pub use self::process::{new_process_handle, Process, ProcessHandle};
pub use self::process_registry::ProcessRegistry;
pub use self::props::Props;
pub use self::restart_statistics::RestartStatistics;
pub use self::spawner::Spawner;
pub use self::supervisor::{SupervisorStrategy, SupervisorStrategyHandle};
pub use self::system::ActorSystem;
pub use self::types::*;
