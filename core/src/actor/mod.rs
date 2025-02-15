//! Actor module provides core actor system functionality.

pub mod actor;
pub mod actor_error;
pub mod actor_system;
pub mod context;
pub mod dispatch;
pub mod event_stream;
pub mod guardian;
pub mod message;
pub mod metrics;
pub mod pid;
pub mod process;
pub mod process_registry;
pub mod props;
pub mod restart_statistics;
pub mod spawner;
pub mod supervisor;
pub mod typed_context;

// Re-export key types and traits
pub use self::{
  actor::*,
  actor_error::*,
  actor_system::ActorSystem,
  context::{
    actor_context::{
      ActorContext, Context, ExtensionPart, InfoPart, MessagePart, ReceiverContext, ReceiverPart, RootContext,
      SenderContext, SenderPart, SpawnerContext, SpawnerPart, StopperPart,
    },
    typed_context::TypedContext,
  },
  message::{Message, MessageHandle, MessageHeaders, MessageOrEnvelope, TypedMessageEnvelope},
  pid::Pid,
  process::{Process, ProcessHandle},
  props::Props,
  restart_statistics::*,
  spawner::*,
  supervisor::*,
};
