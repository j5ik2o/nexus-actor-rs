//! Actor module provides core actor system functionality.

pub mod actor;
pub mod actor_error;
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
pub mod system;

// Re-exports
pub use self::{
  actor::Actor,
  actor_error::ActorError,
  context::{
    ActorContext, Context, MessagePart, ReceiverContext, ReceiverPart, RootContext, SenderContext, SenderPart,
    SpawnerContext, SpawnerPart, StopperPart,
  },
  message::{Message, MessageHandle, MessageHeaders, MessageOrEnvelope, TypedMessageEnvelope},
  pid::Pid,
  process::{Process, ProcessHandle},
  props::Props,
  spawner::SpawnError,
  supervisor::SupervisorStrategy,
  system::ActorSystem,
};

// Convert SpawnError to ActorError
impl From<SpawnError> for ActorError {
  fn from(error: SpawnError) -> Self {
    ActorError::SpawnFailed(error.to_string())
  }
}

// Implement Clone for ProcessHandle
impl Clone for Box<dyn Process + Send + Sync> {
  fn clone(&self) -> Self {
    Box::new(self.as_ref().clone())
  }
}

// Add clone_box method to Process trait
pub trait ProcessClone {
  fn clone_box(&self) -> Box<dyn Process + Send + Sync>;
}

impl<T> ProcessClone for T
where
  T: Process + Clone + Send + Sync + 'static,
{
  fn clone_box(&self) -> Box<dyn Process + Send + Sync> {
    Box::new(self.clone())
  }
}
